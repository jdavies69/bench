# Optional AI budget design

Status: proposal only. Bench is a free MIT native application using the user’s OpenRouter key. This proposal adds no Bench account, billing service, markup, upgrade plan or paid feature gate. No comparison, paid inference or search was run to prepare it.

An optional monthly target makes sense: it gives users a reason to choose economical models for routine work and spend more when a result matters. Start with transparent estimates and deliberate stronger-model retries. Do not start with an invisible classifier that claims to know which model will succeed, or a promised monthly bill ceiling Bench cannot enforce.

## Small native experience

Add an optional **AI budget** section to Settings, collapsed unless enabled: monthly target in USD, **Ask before exceeding target**, and preferred text mode **Economical / Balanced / Best available**. Keep the current explicit model choice available. Default remains the user’s selected model and budget off. Define the month as a UTC calendar month and show that label; preserve historical months when settings change. The target is a user preference, not prepaid credit or a subscription.

Before a paid request, Bench shows a compact estimate only when useful: a stronger-model retry, comparison, unusually large context, media generation, paid tool use or approaching the target. Example copy: “Estimated $0.04–$0.10 · Month target $20.” Those numbers would come from current request/pricing data, never invented examples presented as real results. The request menu offers **Use economical model**, **Use chosen model**, or **Cancel**. At the target it offers an explicit one-request override; it does not silently raise the target or permanently change routing.

Keep sparse native design: one optional settings section, a short estimate adjacent to the relevant action, and a single review sheet when required. Keep provider usage and Bench-only estimated usage visibly distinct. The existing Settings refresh is key-wide provider usage; a new local ledger is needed before claiming Bench-only usage.

## Three different controls

| Control | What it means | What it cannot promise |
| --- | --- | --- |
| Monthly target | Warn or pause Bench when its recorded cost plus pending estimates approaches the user’s target | A ceiling on the OpenRouter account, other apps, another device or unreported charges |
| Bench admission allowance | Reserve a conservative amount before each Bench request; reject requests that cannot fit or cannot be bounded | Exact provider billing when pricing/token accounting is incomplete; enforcement outside this installation |
| Provider key limit | OpenRouter’s own configured spending restriction on the connected key | That the key belongs exclusively to Bench, that all costs are included, or that Bench changed the limit |

OpenRouter’s [current-key endpoint](https://openrouter.ai/docs/api/api-reference/api-keys/get-current-api-key) returns usage, limit, remaining amount, reset interval and separate upstream-provider BYOK usage fields. These values describe that key and may include other applications. Do not sum fields without confirming their accounting meaning. Bench “bring your OpenRouter key” is different from OpenRouter’s “BYOK” feature, which means using upstream provider keys through OpenRouter.

A user who wants provider-enforced limits should configure a dedicated Bench key in OpenRouter and inspect its reset/accounting settings. OpenRouter’s [key update API](https://openrouter.ai/docs/api/api-reference/api-keys/update-keys) documents limits and reset options, but Bench should not request management credentials or change account-wide settings for this first feature. Show a fixed link to the key settings and manual refresh instead. A local target must never be labeled “guaranteed maximum spend.”

## Lean routing and retries

Use a small maintained set of eligible text models with verified capabilities and current prices. First filter by modality, context, structured-output/tools requirements and user restrictions; then choose among economical/balanced/stronger candidates within the request allowance. A pinned model stays pinned unless the user accepts an alternative. Image and Voice use their own compatible catalogs and billable units; a text-model ranking cannot choose a media endpoint safely.

Difficulty starts as a user choice or deterministic task signal: long context, requested structured output, tool workflow, or failed schema validation. These signals identify requirements, not intelligence or guaranteed correctness. Avoid paying another model merely to classify every prompt. Begin routine text with the selected mode; offer **Try a stronger model** after the user rejects the answer or a validator reports a concrete issue. Show its additional estimate and ask before the extra call. Record both attempts separately and preserve the first result. A transient network failure is not evidence that a more expensive model is needed.

OpenRouter already offers [Auto Router](https://openrouter.ai/docs/guides/routing/routers/auto-router), including task/capability-aware selection and cost tiers. Its tier is a band, not a spending ceiling. It can be an optional later alternative after compatibility and spend controls are tested; it should not replace Bench’s explicit user consent or local accounting. Actual resolved model must be recorded when supplied.

For supported text requests, [provider routing](https://openrouter.ai/docs/guides/routing/provider-selection#max-price) exposes `provider.max_price` to filter endpoints by accepted unit prices. Combine this with explicit output token and tool-round limits; unit-price filtering alone is not a total request dollar cap. Do not silently drop a cap to obtain a response or allow a fallback whose prices/capabilities cannot be bounded. If no eligible route exists, explain that the limit prevented the request and offer a reviewable alternative. Capability support must be checked separately for each media API rather than assuming all text routing fields apply.

## Native accounting and missing-usage behavior

Create a local Rust/SQLite request ledger before budget admission. Store a random attempt ID, conversation ID, output kind, model and resolved provider when available, pricing snapshot/version, request/output limits, integer micro-dollar reservation, provider generation ID, timestamps and status. Do not persist the key, raw authorization headers or request bodies in the budget ledger. Conversation content stays in its existing storage. Retries and multi-stage operations each have their own entries; Voice script drafting plus synthesis and Agent/tool calls must all be counted.

Parse catalog decimal price strings without floating-point budget arithmetic. The [model catalog](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties) exposes pricing and capabilities, but advertised model pricing is not sufficient proof of every endpoint, context tier, image/audio unit, search fee or cache charge. Reserve from validated applicable pricing and bounded billable quantities, round upward to integer micro-dollars, and include every possible paid step. A compatible tokenizer or provider-supported admission estimator is required for any claimed input token bound; character count or UTF-8 bytes alone must not be advertised as a universally safe tokenizer bound. Use a displayed range for uncertain estimates. In strict admission mode, unknown pricing or unbounded quantities blocks the request instead of producing a fake upper bound.

Reserve atomically before network dispatch and keep the reservation durable across crashes. Check monthly recorded cost plus all active/unknown reservations in the same transaction, so concurrent windows cannot each spend the same remaining allowance. Changing model/pricing/context invalidates a prior approval or reservation; the confirmed request must bind to the exact plan. Avoid automatic replay after a timeout, because the provider may have completed and billed it.

When valid usage/cost is returned, reconcile that attempt once, preserving estimated versus provider-reported provenance. A missing cost is **unknown**, never $0. A timeout/cancelled request retains its conservative reservation until trustworthy reconciliation or a deliberate user acknowledgment; acknowledgment changes local accounting status, not history or provider charges. If cost exceeds the reservation, record the overage, pause subsequent strict-mode paid requests, and explain the discrepancy. Key-wide usage refresh cannot attribute the delta to one request when another app uses the key. Generation lookup, if supported and authorized, must be bounded, deduplicated and use a fixed provider endpoint; do not infer final cost from the produced text alone.

Settings should show “Recorded $X · Estimated pending $Y · Unknown N” with provenance and last refresh time. Loss of connectivity, stale pricing or malformed provider usage produces an explicit unavailable state. Target-only mode can offer a reviewed estimate override; strict admission mode fails closed when an enforceable bound is unavailable. Neither mode promises a provider-wide bill ceiling.

## Opt-in real model comparisons

Offer **Compare with stronger model** on a completed text response. Show the exact extra models, maximum number of calls, estimated additional range and the local allowance before authorization. Use the same original prompt/context, not the first model’s answer, and clearly disclose any changed context. Default to one additional model and no search/tools. Comparisons that would need paid tools require their own exact approval. The user can keep either result; Bench does not call an LLM judge automatically.

Display the actual outputs side by side with model, elapsed time and reported cost or an explicit unknown/estimated label. User preference can inform a local per-task recommendation; one trial cannot establish that one model is universally better. No hidden benchmark traffic, synthetic claims, promotional ranking or stronger-model upsell. Developer testing uses fixtures unless a specific bounded live comparison is authorized.

## Small implementation milestones and acceptance

1. **Ledger and estimate provenance.** Extend provider results to preserve generation IDs and usage when supplied for all paid paths. Fixture tests prove missing/negative/malformed cost is not free, duplicate events settle once, multi-stage Voice/Agent/search attempts are counted, and crash recovery retains reservations. Existing key-wide usage UI remains correctly labeled.
2. **Optional monthly target.** Add native settings plus one-request review. Tests cover UTC rollover, key replacement, target reduction below recorded usage, concurrent reservation admission, rounding boundaries and settings persistence. A visual review confirms no extra clutter when off.
3. **Explicit economical/stronger choices.** Validate current model capabilities/pricing, bound output and paid tool steps, bind review to the exact request, and reject unavailable capped routes. Fixtures prove stale/unknown pricing and fallback cannot bypass strict admission. Preserve the original response when a stronger attempt fails.
4. **One real comparison after authorization.** Use an agreed prompt, two eligible models and a specific total allowance; record actual responses/provenance and cost uncertainty. No live comparison is part of this design task. Accept only after the user can reproduce the displayed choice and both results survive restart.

Defer automatic difficulty classifiers, automatic quality judging, broad benchmarks, management-key provisioning and account synchronization. The first useful product is understandable cost guidance and deliberate retries; stricter local admission follows only where costs can be bounded honestly.
