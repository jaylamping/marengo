# Batch25 — research rolling publication windows

T20 remains open. Baselinead09ee4, branch codex/research-rolling-windows.
A public offline orchestrator probe fails three original assertions: unknown dates
are returned as satisfying week, month and year requests. The any-recency positive
passes. Complete probe bytes and baseline are frozen under evidence/batch25; no
provider network call occurs. Original utcnow deprecation warnings are not counted
as behavioral assertion failures.

Full scope remains pending: preserve optional provider publication/update timestamps
and timezone semantics through normalization, apply actual rolling UTC week/month
windows with injected clock, declare unknown-date retention/exclusion, and qualify
boundary instants, future/missing dates and year rollover through public responses.
Unknown dates will remain eligible for any-recency requests and be excluded from
restricted windows. Provider normalization, implementation, independent reviews,
primary qualification and delivery remain pending. No robot, deploy, limits, Wave
or automation action.

## Implementation qualification in progress

Provider normalization now retains optional aware `published_at` and `updated_at`.
Restricted windows compare inclusive UTC instants over 7/30/365 days; unknown and
future dates are excluded. Any-recency retains all results. Code/Hub artifacts use
last code/activity date with creation fallback; papers/posts use publication only,
so a modified old paper cannot become a new publication. Date-only provider values
have declared midnight-UTC precision. Malformed/naive date-times are unknown.

The original public four-case probe is byte-identical and green. The complete
locked offline suite passes44 tests with warnings treated as errors. Additional
public JSON checks qualify inclusive boundaries, a microsecond outside, future
instants, year-old and missing dates with a fixed clock crossing year rollover.
Provider response fixtures exercise GitHub, Semantic Scholar, OpenReview, Reddit,
Papers With Code and Hugging Face normalization through actual orchestration;
existing arXiv adapter qualification remains, additional timestamp coverage pending.
Timezone wire round-trip and paper-update/code-activity distinctions also pass.

Provider facts checked against primary sources:
- [OpenReview fields](https://docs.openreview.net/reference/api-v1/entities/note/fields):
  `odate` first public, `pdate` acceptance, epoch milliseconds. Use `odate`, avoiding
  false freshness from recent acceptance of an old public paper.
- [Hugging Face client](https://huggingface.co/docs/huggingface_hub/main/en/package_reference/hf_api):
  `createdAt` and `lastModified`; request projection/default response coverage remains under review.
- [Papers With Code original model](https://github.com/paperswithcode/paperswithcode-client/blob/master/paperswithcode/models/paper.py):
  `published` optional calendar date. This is schema qualification, not a claim of live availability.
- [Semantic Scholar tutorial](https://webflow.semanticscholar.org/product/api/tutorial):
  `publicationDate` is a requested paper field.

Standards and Spec reviews are active against efc21b6; later9f3ce1b adds only public
boundary assertions/evidence. Native provider date filters, arXiv timestamp fixture,
full review, primary gate coverage and delivery remain pending. T20 remains open.

## Independent review findings and repair

Standards: no documented hard breaches; dated cache serialization integration
failure and a possible duplicated window-day policy. Spec: same cache integration
failure, missing supported provider temporal filters, and public boundary coverage
missing at the reviewed efc21b6. The later9f3ce1b supplies public boundary coverage.
At14cdcf2 the actual cold/hot owned disk cache regression reproduces a datetime
serialization TypeError before repair; unchanged test then passes with JSON-mode
serialization. Window policy is centralized. Complete offline suite now45 passes,
with warnings treated as errors. Public async handler await repair remains T17
work; this cache test proves the dated value path without claiming T17 closure.
Supported provider filters and final review/gates/delivery remain pending.

At40984f1, native GitHub pushed, Semantic Scholar publicationDateOrYear and arXiv
submittedDate ranges use one captured UTC clock; their provider precision is a
superset, with exact local filtering retained. Actual request fixtures qualify the
ranges; arXiv timestamp/public response coverage now included. At2bd149f Hub date
fields are explicitly projected through `expand`. Full locked offline46 passes.
Second review active. G19 exact merged-main five-job CI passed36903696586 and is
recorded verified in the ledger (22 verified,11 partial,69 open). T20 remains open.
