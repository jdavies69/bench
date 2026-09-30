import { createHash } from 'node:crypto';

export class GenerationPolicyError extends Error {
  constructor(code, message) { super(message); this.name = 'GenerationPolicyError'; this.code = code; }
}

const reject = (code, message) => { throw new GenerationPolicyError(code, message); };
const object = (value) => value !== null && typeof value === 'object' && !Array.isArray(value) && [Object.prototype, null].includes(Object.getPrototypeOf(value));
const exactKeys = (value, keys) => object(value) && Object.keys(value).length === keys.length && keys.every((key) => Object.hasOwn(value, key));
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const safeInteger = (value, minimum = 1) => Number.isSafeInteger(value) && value >= minimum;
const hash = (value) => createHash('sha256').update(JSON.stringify(value)).digest('hex');

function context(request) {
  return { conversationId: request.conversationId.toLowerCase(), userMessageId: request.userMessageId.toLowerCase(), output: request.output, messages: request.messages.map(({ role, content }) => ({ role, content })) };
}

// Canonical field order; content bytes/whitespace are preserved, never trimmed.
// Account identity is supplied by authenticated service context, never this body.
export function hashGenerationContext(request) { return hash(context(request)); }

export function validateGeneration(request, plan) {
  if (!exactKeys(request, ['attemptId', 'conversationId', 'userMessageId', 'output', 'contextHash', 'messages'])) reject('invalid_request', 'Invalid generation request fields.');
  for (const key of ['attemptId', 'conversationId', 'userMessageId']) {
    if (typeof request[key] !== 'string' || !uuid.test(request[key])) reject('invalid_request', 'Invalid generation identifier.');
  }
  if (!['chat', 'website'].includes(request.output)) reject('invalid_request', 'Unsupported generation output.');
  if (typeof request.contextHash !== 'string' || !/^[a-f0-9]{64}$/.test(request.contextHash)) reject('invalid_request', 'Invalid context digest.');
  if (!object(plan) || !safeInteger(plan.maxMessages) || !object(plan.outputModels) || !object(plan.models)) reject('invalid_configuration', 'Generation policy is unavailable.');
  if (!Array.isArray(request.messages) || request.messages.length < 1 || request.messages.length > plan.maxMessages) reject('context_limit', 'Conversation context exceeds the allowed size.');
  const model = Object.hasOwn(plan.outputModels, request.output) ? plan.outputModels[request.output] : undefined;
  const config = typeof model === 'string' && Object.hasOwn(plan.models, model) ? plan.models[model] : undefined;
  if (!object(config) || !safeInteger(config.inputMicrosPerMillionTokens, 0) || !safeInteger(config.outputMicrosPerMillionTokens, 0) || !safeInteger(config.maxInputBytes) || !safeInteger(config.maxInputTokens) || !safeInteger(config.maxOutputTokens) || typeof config.estimateInputTokens !== 'function') reject('invalid_configuration', 'Generation policy is unavailable.');
  if (typeof plan.maxReserveMicros !== 'string' || !/^[1-9][0-9]*$/.test(plan.maxReserveMicros) || plan.maxReserveMicros.length > 100) reject('invalid_configuration', 'Generation policy is unavailable.');
  if (BigInt(plan.maxReserveMicros) > 9_223_372_036_854_775_807n) reject('invalid_configuration', 'Generation policy is unavailable.');
  let utf8Bytes = 0;
  for (const message of request.messages) {
    if (!exactKeys(message, ['role', 'content']) || !['system', 'user', 'assistant'].includes(message.role) || typeof message.content !== 'string' || !message.content.length) reject('invalid_request', 'Invalid conversation message.');
    utf8Bytes += Buffer.byteLength(message.content, 'utf8');
    if (!Number.isSafeInteger(utf8Bytes) || utf8Bytes > config.maxInputBytes) reject('context_limit', 'Conversation context exceeds the allowed size.');
  }
  if (request.messages.at(-1).role !== 'user') reject('invalid_request', 'Generation context must end with a user message.');
  const canonical = context(request);
  if (hash(canonical) !== request.contextHash) reject('context_conflict', 'Conversation context does not match its digest.');
  let inputTokens;
  try {
    // Trusted server adapter must bound the complete provider prompt, including
    // role framing and any server-added content. UTF-8 bytes are an admission
    // limit, NOT a universal tokenizer upper bound. Missing certification fails
    // closed rather than assuming a model's tokenizer/cost behavior.
    inputTokens = config.estimateInputTokens({ messages: canonical.messages, output: request.output, utf8Bytes });
  } catch { reject('invalid_configuration', 'Input admission estimation is unavailable.'); }
  if (!safeInteger(inputTokens, 0)) reject('invalid_configuration', 'Input admission estimation is unavailable.');
  if (inputTokens > config.maxInputTokens) reject('context_limit', 'Conversation context exceeds the allowed size.');
  const ceilMillion = (tokens, rate) => (BigInt(tokens) * BigInt(rate) + 999_999n) / 1_000_000n;
  const reserve = ceilMillion(inputTokens, config.inputMicrosPerMillionTokens) + ceilMillion(config.maxOutputTokens, config.outputMicrosPerMillionTokens);
  if (reserve === 0n) reject('invalid_configuration', 'Generation policy requires a positive reservation.');
  if (reserve > BigInt(plan.maxReserveMicros)) reject('reservation_limit', 'Generation exceeds the configured request allowance.');
  const normalizedRequest = { attemptId: request.attemptId.toLowerCase(), ...canonical, contextHash: request.contextHash, maxOutputTokens: config.maxOutputTokens };
  // Bind the server-selected model/output cap too, so a retry after a policy
  // change cannot silently launch a different potentially billable request.
  // Accepted reservation/pricing metadata must remain immutable in the ledger.
  // Retry lookup uses persisted acceptance metadata, never current price rates.
  const requestDigest = hash({ normalizedRequest, model });
  return { normalizedRequest, requestDigest, model, reserveMicros: reserve.toString() };
}
