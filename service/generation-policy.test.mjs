import test from 'node:test';
import assert from 'node:assert/strict';
import { GenerationPolicyError, hashGenerationContext, validateGeneration } from './generation-policy.mjs';

function fixture() {
  const request = { attemptId: '12e45678-e89b-42d3-a456-426614174000', conversationId: '12e45678-e89b-42d3-a456-426614174001', userMessageId: '12e45678-e89b-42d3-a456-426614174002', output: 'chat', messages: [{ role: 'user', content: 'hello' }] };
  request.contextHash = hashGenerationContext(request);
  const plan = { maxMessages: 10, maxReserveMicros: '9223372036854775807', outputModels: { chat: 'test/model', website: 'test/model' }, models: { 'test/model': { inputMicrosPerMillionTokens: 1_000_000, outputMicrosPerMillionTokens: 2_000_000, maxInputBytes: 100, maxInputTokens: 120, maxOutputTokens: 10, estimateInputTokens: ({ utf8Bytes }) => utf8Bytes + 8 } } };
  return { request, plan, config: plan.models['test/model'] };
}
const rejects = (code, action) => assert.throws(action, (error) => error instanceof GenerationPolicyError && error.code === code);

test('server chooses configured model and bounded output with exact integer reserve', () => {
  const { request, plan } = fixture();
  const result = validateGeneration(request, plan);
  assert.equal(result.reserveMicros, '33');
  assert.equal(result.model, 'test/model');
  assert.equal(result.normalizedRequest.maxOutputTokens, 10);
  assert.equal(result.normalizedRequest.messages[0].content, 'hello');
  assert.match(result.requestDigest, /^[a-f0-9]{64}$/);
  assert.equal(Object.hasOwn(request, 'maxOutputTokens'), false);
});

test('rejects model, output-token, account, tools, and per-message overrides', () => {
  for (const extra of ['model', 'maxOutputTokens', 'accountId', 'tools', '__proto__']) {
    const { request, plan } = fixture();
    Object.defineProperty(request, extra, { value: 'attacker', enumerable: true });
    rejects('invalid_request', () => validateGeneration(request, plan));
  }
  const { request, plan } = fixture();
  request.messages[0].tool_calls = [];
  rejects('invalid_request', () => validateGeneration(request, plan));
});

test('UTF-8 bytes enforce multibyte limits before estimator and include message framing admission', () => {
  const { request, plan, config } = fixture();
  request.messages[0].content = '😀'.repeat(25);
  request.contextHash = hashGenerationContext(request);
  assert.equal(validateGeneration(request, plan).reserveMicros, '128');
  request.messages[0].content += 'a';
  request.contextHash = hashGenerationContext(request);
  config.estimateInputTokens = () => { throw new Error('must not be called'); };
  rejects('context_limit', () => validateGeneration(request, plan));
});

test('exact boundaries pass, changed context/id/output rejects, canonical order is stable', () => {
  const { request, plan, config } = fixture();
  config.maxInputBytes = 5; config.maxInputTokens = 13;
  assert.equal(validateGeneration(request, plan).reserveMicros, '33');
  assert.equal(validateGeneration({ ...request, messages: [{ content: 'hello', role: 'user' }] }, plan).requestDigest, validateGeneration(request, plan).requestDigest);
  for (const change of [{ conversationId: '12e45678-e89b-42d3-a456-426614174009' }, { output: 'website' }, { messages: [{ role: 'user', content: 'HELLO' }] }]) rejects('context_conflict', () => validateGeneration({ ...request, ...change }, plan));
  config.maxInputTokens = 12;
  rejects('context_limit', () => validateGeneration(request, plan));
});

test('rounds each price component upward and rejects reserve one unit over cap', () => {
  const { request, plan, config } = fixture();
  config.inputMicrosPerMillionTokens = 1; config.outputMicrosPerMillionTokens = 1;
  assert.equal(validateGeneration(request, plan).reserveMicros, '2');
  plan.maxReserveMicros = '2'; validateGeneration(request, plan);
  plan.maxReserveMicros = '1'; rejects('reservation_limit', () => validateGeneration(request, plan));
});

test('BigInt reservation remains exact above the Number safe-integer boundary', () => {
  const { request, plan, config } = fixture();
  config.inputMicrosPerMillionTokens = Number.MAX_SAFE_INTEGER;
  config.outputMicrosPerMillionTokens = Number.MAX_SAFE_INTEGER;
  config.maxOutputTokens = 2000000;
  plan.maxReserveMicros = '9223372036854775807';
  const expected = (13n * BigInt(Number.MAX_SAFE_INTEGER) + 999999n) / 1000000n + (BigInt(Number.MAX_SAFE_INTEGER) * 2000000n + 999999n) / 1000000n;
  assert.equal(validateGeneration(request, plan).reserveMicros, expected.toString());
});

test('missing/invalid/throwing estimator and untrusted pricing configuration fail closed', () => {
  for (const estimator of [undefined, () => 1.5, () => -1, () => Promise.resolve(1), () => { throw new Error('private details'); }]) {
    const { request, plan, config } = fixture(); config.estimateInputTokens = estimator;
    rejects('invalid_configuration', () => validateGeneration(request, plan));
  }
  for (const rate of [-1, 0.1, Number.MAX_SAFE_INTEGER + 1, '1000']) {
    const { request, plan, config } = fixture(); config.outputMicrosPerMillionTokens = rate;
    rejects('invalid_configuration', () => validateGeneration(request, plan));
  }
});

test('invalid UUIDs, roles, empty context and excessive messages reject without reflecting content', () => {
  const { request, plan } = fixture();
  for (const change of [{ attemptId: 'not-a-uuid' }, { output: 'image' }, { contextHash: 'bad' }, { messages: [] }, { messages: Array(11).fill({ role: 'user', content: 'secret' }) }, { messages: [{ role: 'assistant', content: 'secret' }] }, { messages: [{ role: 'tool', content: 'secret' }] }]) {
    assert.throws(() => validateGeneration({ ...request, ...change }, plan), (error) => error instanceof GenerationPolicyError && !error.message.includes('secret'));
  }
});

test('server output-cap/model changes bind a different request digest for retry safety', () => {
  const { request, plan, config } = fixture();
  const first = validateGeneration(request, plan).requestDigest;
  config.maxOutputTokens += 1;
  assert.notEqual(validateGeneration(request, plan).requestDigest, first);
});

test('zero-cost reservation and configuration above signed 64-bit ledger bound fail closed', () => {
  const { request, plan, config } = fixture();
  config.inputMicrosPerMillionTokens = 0; config.outputMicrosPerMillionTokens = 0;
  rejects('invalid_configuration', () => validateGeneration(request, plan));
  config.inputMicrosPerMillionTokens = 1000;
  plan.maxReserveMicros = '9223372036854775808';
  rejects('invalid_configuration', () => validateGeneration(request, plan));
  plan.maxReserveMicros = '9223372036854775807';
  assert.equal(validateGeneration(request, plan).reserveMicros, '1');
});
