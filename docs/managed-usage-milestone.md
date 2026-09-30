# Optional accounts and Bench-managed usage

Status: deferred, September 30, 2026. The latest user choice is BYOK first for the bare-minimum Apple Silicon release. A future managed plan would use a monthly fixed balance and cost pass-through with roughly 5% markup; Supabase and Stripe were approved for that future backend, but no Bench-owned accounts exist. Identity, hosted billing, managed desktop access, and live managed provider calls are unavailable. This document does not provision any account or payment service.

## Chosen product model

| Mode | Sign-in | Payment | History and project files |
| --- | --- | --- | --- |
| Bring your own key (current behavior) | Optional | User pays the API provider or OpenRouter account they connect | Local SQLite and app-data workspace |
| Bench managed | Required | Monthly Bench subscription with included usage | Still local SQLite and app-data workspace |

The first managed plan is a **fixed monthly price with a hard included-usage cap**. There is no automatic overage charge, pay-as-you-go bill, or unlimited inference. When the allowance is exhausted, managed requests stop until the next paid period. Optional top-ups can be considered later; they are not part of this milestone. Set the plan price, allowance, allowed models, and exact customer-facing usage unit before building checkout. The allowance should cap Bench's worst-case provider spend with room for payment costs and support.

## Existing bring-your-own-key paths

These paths use the user's own API billing or credits. None requires a Bench account, creates a Bench subscription, or makes Bench collect payment:

| Connection | Where inference is billed | Bench behavior |
| --- | --- | --- |
| Direct OpenAI, Anthropic, or xAI API key | The user's provider API account | Already supported; the key stays in the macOS Keychain. |
| User-funded OpenRouter API key | The user's OpenRouter credits | Already supported; OpenRouter OAuth could later simplify connecting that account. |
| OpenRouter API key with a provider API key configured in OpenRouter BYOK | The user's upstream provider API account; OpenRouter may charge a BYOK routing fee after its plan allowance | Bench uses the user's OpenRouter key as it does today. The user configures the upstream key in OpenRouter, not Bench. |

The last path works with [OpenAI, Anthropic, and xAI provider keys](https://openrouter.ai/providers/). There is no documented path to transfer unused ChatGPT Plus/Pro, Claude Pro/Max, or Grok consumer subscription allowances into OpenRouter. OpenRouter documents [provider API keys](https://openrouter.ai/docs/guides/overview/auth/byok), not consumer subscription entitlements; this is the supported distinction, not a claim that future provider programs could never change it. OpenRouter's [OAuth flow](https://openrouter.ai/docs/guides/overview/auth/oauth) creates a user-controlled **OpenRouter** API key and uses that user's OpenRouter billing.

OpenRouter's BYOK routing may [fall back to shared OpenRouter capacity](https://openrouter.ai/docs/guides/overview/auth/byok) when an upstream key fails or cannot serve a model. That fallback spends the user's OpenRouter credits. A provider-only billing promise therefore requires the user to disable shared-capacity fallback for that provider and restrict routing appropriately. OpenRouter's [current pricing](https://openrouter.ai/pricing) lists a plan-dependent no-fee BYOK allowance, followed by a 5% routing fee; do not assume a permanent flat fee or zero fee. These external balances and charges are separate from the proposed Bench-managed monthly allowance.

Switching modes does not migrate, delete, or upload existing conversations. Direct OpenRouter, OpenAI, Anthropic, and xAI keys remain in the macOS Keychain through `key_store.rs`; they never go to Bench servers. A managed request does transmit the conversation context needed for that turn to Bench's service and then OpenRouter. The app must state this clearly before enabling managed mode. The service need not retain prompt or response bodies after delivery; it retains metering and error metadata.

## Existing seams to preserve

- `src/native.ts` is the React-to-Tauri boundary. React never receives model credentials.
- `src-tauri/src/provider.rs` owns `ModelProvider` and direct-provider streaming. `commands.rs` selects the provider, persists the user's prompt before a request, and saves only a complete assistant response. Website generation also consumes `ModelProvider`.
- `src-tauri/src/key_store.rs` owns local BYOK credentials. `src-tauri/src/db.rs` owns local projects, conversations, messages, and settings. Neither becomes the source of truth for subscription status or monthly allowance.
- Web search is a separate `Tool`; payment mode must not become a tool provider or couple tool execution to OpenRouter.

The smallest code extension is a `model_access_mode` preference (`byok` or `managed`) alongside the existing local provider preference. A `ManagedProvider` should implement the same model-provider interface for Chat and Website calls. Extend the interface's completion result to carry metering metadata (`request_id`, generation ID, actual cost or reconciliation status) without exposing credentials to React. In managed mode, the Rust layer connects to Bench over HTTPS with a session token stored in the system credential store. The OpenRouter key is never put into Tauri resources, environment defaults, SQLite, or a frontend bundle.

## Server boundary and subscription flow

The managed service needs a durable transactional database (for example, Postgres). The desktop's SQLite remains local user data; it cannot safely authorize concurrent usage across devices or sessions.

1. User signs in through a system-browser flow. The service returns a short-lived access token and rotating refresh credential; Rust stores the refresh credential in the system credential store. Local BYOK keeps working when signed out; local history remains readable offline.
2. The service creates a Stripe Checkout Session in `subscription` mode for one fixed monthly price. The app opens hosted Checkout. Its return URL is not proof of payment.
3. A verified paid subscription invoice grants exactly one allowance for that Stripe subscription period, keyed by invoice or period ID so duplicate and out-of-order webhooks cannot grant it twice. The first successful invoice starts access; each paid renewal creates the next period's allowance. The old allowance expires at its period end and does not roll over. Never reset usage just because the local calendar month changed.
4. Rust sends a managed request to `POST /v1/generations` with account session, local conversation ID, unique attempt ID, output type, and minimum model context. The server confirms a currently paid period, validates the model and size limits, and atomically reserves enough of that period's remaining allowance for the bounded request before contacting OpenRouter.
5. The server streams deltas to Rust, which uses the existing UI channel. The client keeps its complete-only assistant-message commit behavior. The server records the OpenRouter generation ID and actual cost, settles the reservation against the published allowance schedule, and releases unused allowance. A failed stream with unknown cost stays pending for background reconciliation before release. Idempotency prevents duplicate starts and debits for one attempt.
6. `GET /v1/entitlement` returns plan, billing period end, available and reserved allowance, and recent usage. The app shows a quiet usage state and manage-plan action in Connections. At the cap, it explains that managed usage resumes at the next paid renewal; the user's prompt remains in SQLite and BYOK remains available.

On a renewal payment failure, do **not** grant a new allowance. The current paid period remains valid only until its end; then pause managed requests until the invoice is paid. Show a concise payment issue and a path to update the payment method. A later successful payment grants that period once, after reconciling Stripe's subscription and invoice state. Cancellation at period end keeps the already-paid allowance until that end, then stops renewal. Refunds, disputes, immediate cancellation, and plan changes need explicit entitlement adjustments and must not leave an unfunded allowance active. No automatic overage invoice is ever created in this release.

The server should keep `accounts`, `subscriptions`, `subscription_periods`, `stripe_events`, `generation_attempts`, and an append-only `allowance_ledger`. Store amounts as integer micro-units or exact decimals, never floating-point balance arithmetic. Each invoice/period grant, reservation, settlement, and reversal needs a unique reference. The allowance check and reservation must happen in one database transaction with a row lock or equivalent atomic update. `generation_attempts` should store account ID, local conversation ID, request ID, model, status, generation ID, input/output tokens, OpenRouter cost, allowance debit, and timestamps; prompt text is not needed for the billing ledger.

Suggested endpoints: account sign-in callback/session refresh, `POST /v1/subscription-checkout`, `POST /v1/stripe/webhook`, `GET /v1/entitlement`, `POST /v1/generations` (streaming), and a Stripe customer-portal session for payment-method updates and cancellation. No conversation-sync API is needed.

## Spend and abuse controls

- Start with a short model allowlist and maximum input size, output tokens, per-request estimated cost, concurrent requests, and daily spend. Apply limits to Website generation as well as Chat. Reject requests that cannot fit within the remaining allowance; do not let usage overrun and bill later.
- Keep an OpenRouter account/workspace cap and, if useful, server-held per-user completion keys with OpenRouter limits as a second layer. OpenRouter management keys can create and limit completion keys, but those limits do not replace Bench's transactional allowance ledger. Never issue such keys to desktop clients.
- Require TLS, authenticate every managed call, rate-limit account and network origin, redact prompts and secrets from logs, and rotate server credentials. The service owns OpenRouter and Stripe secrets. Desktop tokens have no Stripe or OpenRouter administrative authority.
- Reconcile OpenRouter actual usage against generation IDs, including client disconnects, provider failures, and process restarts. Failed or zero-cost requests release their reserve once verified. Unknown-cost attempts remain held and visible for support review, rather than silently restoring allowance or double-debiting it.
- Verify Stripe webhook signatures against the raw request body, deduplicate event IDs, and query current Stripe objects when events arrive out of order. Handle `invoice.paid`, `invoice.payment_failed`, and subscription update/deletion events. Only a confirmed paid invoice grants a new period; Stripe is the payment processor and Bench's ledger is the immediate authorization source.

## Delivery order and acceptance

1. **Auth and mode boundary:** optional account sign-in/sign-out, BYOK/managed choice, secure session storage, and no change to existing Keychain/SQLite data. Signed-out BYOK remains usable.
2. **Entitlement and test subscription:** server migrations, Stripe test-mode monthly Checkout and signed webhooks, idempotent period grants, allowance display, cancellation, payment-failure pause, and refund reversal. No live charges.
3. **Managed inference:** server-held OpenRouter completion key, streaming managed provider for Chat and Website, reservation and settlement, and clear cap/error states. Existing BYOK paths remain regression-tested.
4. **Failure tests and release gate:** duplicate/out-of-order Stripe events, failed renewal, delayed payment recovery, cancellation at period end, concurrent requests at the last allowance unit, network loss midstream, empty/malformed provider response, server restart before settlement, retry idempotency, sign-out/relaunch, and local conversation reload. Compare the server ledger to OpenRouter generation costs. Only then run one explicitly approved live paid test and consider production payment enablement.

## Decisions and external setup required

The monthly-with-cap model is chosen. These items remain blockers to integration and live payment:

1. **Plan economics:** monthly price, included allowance and its user-facing unit, allowed starter models, per-request cap, whether unused allowance expires (recommended: yes, at period end), cancellation/refund policy, and taxes. The first release has no automatic overages; future top-ups require a separate product decision.
2. **Identity and hosting:** sign-in provider/method (recommend system-browser email link or passkey), Bench service domain, transactional database host, and production secret storage. No Bench backend host/domain is available yet.
3. **OpenRouter:** a Bench-owned account and server-only completion key, funded spend cap, and optionally a management key for server-held per-user keys. None is available yet. Do not reuse a founder's personal BYOK key for customer traffic.
4. **Stripe:** a Bench-owned account, test secret key, monthly recurring Price ID, webhook signing secret, and customer-portal configuration. None is available yet. Production credentials and charges are a separate go-live decision.
5. **Customer terms and data handling:** subscription renewal, usage cap, refund/cancellation wording, taxes, and the disclosure that managed prompts pass through Bench's service. Resolve these before showing an actual purchase flow.

## Source notes

- OpenRouter: [provider API key BYOK and fallback](https://openrouter.ai/docs/guides/overview/auth/byok), [provider BYOK availability](https://openrouter.ai/providers/), [current BYOK pricing](https://openrouter.ai/pricing), [management keys and limits](https://openrouter.ai/docs/guides/overview/auth/management-api-keys), [generation cost metadata](https://openrouter.ai/docs/api/api-reference/generations/get-generation), [in-stream usage accounting](https://openrouter.ai/blog/announcements/smarter-charts-inline-svgs-and-live-usage-accounting/), and [user-funded OAuth](https://openrouter.ai/docs/guides/overview/auth/oauth).
- Stripe: [monthly fixed-rate prices](https://docs.stripe.com/billing/subscriptions/metered-billing/thresholds), [subscription Checkout](https://docs.stripe.com/api/checkout/sessions/create), [subscription and invoice events](https://docs.stripe.com/api/events/types), [subscription payment states](https://docs.stripe.com/api/subscriptions/object), and [signed webhooks](https://docs.stripe.com/webhooks).

OpenRouter OAuth connects a customer's own OpenRouter account and belongs to the BYOK route. It does not collect a Bench payment or authorize Bench-funded inference. Stripe's metered overage and credit-grant models are intentionally outside this first fixed-price, hard-cap plan.
