import { validateGeneration } from './generation-policy.mjs';

export class GatewayError extends Error {
  constructor(code, message) { super(message); this.name = 'GatewayError'; this.code = code; }
}
const reject = (code, message) => { throw new GatewayError(code, message); };
function exactMicros(value) {
  if (typeof value !== 'string' || value.length > 19 || !/^(0|[1-9]\d*)$/.test(value) || BigInt(value) > 9223372036854775807n) reject('INVALID_USAGE', 'Provider usage could not be verified.');
  return value;
}

// Internal coordinator only: caller authenticates accountId and supplies a
// reviewed server plan. Provider/convertCost are trusted server adapters, never
// request parameters. No durable response replay is implemented here.
export function createGateway({ ledger, provider, plan, convertCost = (cost) => cost, maxResponseBytes = 400_000, maxEvents = 100_000, timeoutMs = 240_000 }) {
  if (!ledger || !provider || typeof provider.streamGeneration !== 'function' || typeof convertCost !== 'function' || ![maxResponseBytes, maxEvents, timeoutMs].every((v) => Number.isSafeInteger(v) && v > 0)) throw new TypeError('Invalid gateway configuration.');
  async function generate({ accountId, request, signal }) {
    const checked = validateGeneration(request, plan);
    const { attempt, created } = await ledger.reserve({ accountId, attemptId: checked.normalizedRequest.attemptId, conversationId: checked.normalizedRequest.conversationId, requestDigest: checked.requestDigest, model: checked.model, reserveMicros: String(checked.reserveMicros) });
    if (!created) return { replay: true, attempt, responseAvailable: false };
    const identity = { accountId, attemptId: attempt.attemptId };
    // This is the only zero-cost proof this coordinator can establish: no call
    // to the trusted provider adapter has happened yet.
    if (signal?.aborted) { await ledger.fail({ ...identity, costKnownZero: true }); reject('CANCELLED', 'The request was cancelled before starting.'); }
    let iterator;
    let generationId, content = '', bytes = 0, events = 0, completion;
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener('abort', abort, { once: true });
    let timer;
    const interrupted = new Promise((_, fail) => {
      controller.signal.addEventListener('abort', () => fail(new GatewayError('INTERRUPTED', 'The response was interrupted. Usage may still be reconciling.')), { once: true });
      timer = setTimeout(abort, timeoutMs);
    });
    void interrupted.catch(() => {});
    if (signal?.aborted) controller.abort();
    try {
      const stream = await Promise.race([provider.streamGeneration(checked.normalizedRequest, { model: checked.model, maxOutputTokens: checked.normalizedRequest.maxOutputTokens, signal: controller.signal }), interrupted]);
      if (!stream?.[Symbol.asyncIterator]) reject('INVALID_STREAM', 'The provider returned an invalid response.');
      iterator = stream[Symbol.asyncIterator]();
      while (true) {
        const item = await Promise.race([iterator.next(), interrupted]);
        if (item.done) break;
        if (++events > maxEvents) reject('OVERSIZED_RESPONSE', 'The response was too large.');
        const event = item.value;
        if (!event || typeof event !== 'object' || completion) reject('INVALID_STREAM', 'The provider returned an invalid response.');
        if (event.type === 'started') {
          if (generationId || typeof event.generationId !== 'string' || !event.generationId.trim() || event.generationId.length > 200 || /[\u0000-\u001f]/.test(event.generationId)) reject('INVALID_STREAM', 'Provider generation could not be verified.');
          generationId = event.generationId;
          await ledger.recordGeneration({ ...identity, generationId });
        } else if (event.type === 'delta') {
          if (!generationId || typeof event.text !== 'string') reject('INVALID_STREAM', 'The provider returned an invalid response.');
          bytes += Buffer.byteLength(event.text, 'utf8');
          if (bytes > maxResponseBytes) reject('OVERSIZED_RESPONSE', 'The response was too large.');
          content += event.text;
        } else if (event.type === 'completed') {
          if (!generationId || event.generationId !== generationId || !content.trim()) reject('INVALID_STREAM', 'The provider did not complete the response.');
          completion = { costMicros: exactMicros(event.costMicros), generationId };
        } else reject('INVALID_STREAM', 'The provider returned an invalid response.');
      }
      if (!completion) reject('TRUNCATED_STREAM', 'The provider did not complete the response.');
      const debitMicros = exactMicros(await convertCost(completion.costMicros, { model: checked.model, output: checked.normalizedRequest.output }));
      const settled = await ledger.settle({ ...identity, ...completion, debitMicros });
      return { replay: false, attempt: settled, responseAvailable: true, content };
    } catch (error) {
      controller.abort();
      // Never trust an exception's zero-cost flag: dispatch may already have
      // incurred provider usage, even without a received generation ID.
      await ledger.fail({ ...identity, ...(generationId ? { generationId } : {}) }).catch(() => {});
      if (error instanceof GatewayError) throw error;
      throw new GatewayError('GENERATION_PENDING', 'The response could not be completed. Usage may still be reconciling.');
    } finally {
      clearTimeout(timer); signal?.removeEventListener('abort', abort);
      controller.abort();
      // A stuck adapter must not block recovery while closing its iterator.
      if (iterator?.return) { try { void Promise.resolve(iterator.return()).catch(() => {}); } catch {} }
    }
  }
  async function lookup({ accountId, attemptId }) {
    return { attempt: await ledger.lookupAttempt({ accountId, attemptId }), responseAvailable: false };
  }
  return { generate, lookup };
}
