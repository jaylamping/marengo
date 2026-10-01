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
