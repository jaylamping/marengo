"""Hugging Face Hub search."""

from __future__ import annotations

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit

HF_MODELS = "https://huggingface.co/api/models"
HF_DATASETS = "https://huggingface.co/api/datasets"


async def search_huggingface(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    headers = {"User-Agent": cfg.user_agent}
    params = {"search": f"{query} robotics humanoid", "limit": limit}
    hits: list[ResearchHit] = []
    try:
        async with httpx.AsyncClient(timeout=30.0, headers=headers) as client:
            for url, kind in ((HF_MODELS, "model"), (HF_DATASETS, "dataset")):
                resp = await client.get(url, params=params)
                if resp.status_code != 200:
                    continue
                for item in resp.json()[: max(1, limit // 2)]:
                    model_id = item.get("modelId") or item.get("id", "")
                    if not model_id:
                        continue
                    hits.append(
                        ResearchHit(
                            type="hf",
                            title=model_id,
                            url=f"https://huggingface.co/{model_id}",
                            snippet=f"HF {kind}: {item.get('pipeline_tag', '')}",
                            source_name=f"huggingface/{kind}",
                            stars=item.get("likes"),
                        )
                    )
    except httpx.HTTPError:
        pass
    return hits[:limit]
