import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  buildSaveMetadata,
  filterRowsByTopicKey,
  mergeSaveMetadata,
} from "../src/client.js";

describe("buildSaveMetadata", () => {
  it("sets topic_key and optional fields", () => {
    assert.deepEqual(
      buildSaveMetadata({
        title: "sdd/foo/tasks",
        topicKey: "sdd/foo/tasks",
        type: "architecture",
        project: "marengo",
        capturePrompt: false,
      }),
      {
        topic_key: "sdd/foo/tasks",
        title: "sdd/foo/tasks",
        type: "architecture",
        project: "marengo",
        capture_prompt: false,
      },
    );
  });
});

describe("mergeSaveMetadata", () => {
  it("preserves unrelated metadata while refreshing topic fields", () => {
    assert.deepEqual(
      mergeSaveMetadata(
        { topic_key: "old", source_ref: "mem_save", extra: 1 },
        {
          title: "sdd/foo/tasks",
          topicKey: "sdd/foo/tasks",
          project: "marengo",
        },
      ),
      {
        topic_key: "sdd/foo/tasks",
        title: "sdd/foo/tasks",
        source_ref: "mem_save",
        extra: 1,
        project: "marengo",
      },
    );
  });
});

describe("filterRowsByTopicKey", () => {
  it("matches exact topic_key and optional project", () => {
    const rows = [
      {
        id: "a",
        metadata: { topic_key: "sdd/foo/tasks", project: "marengo" },
      },
      {
        id: "b",
        metadata: { topic_key: "sdd/foo/tasks", project: "other" },
      },
      {
        id: "c",
        metadata: { topic_key: "sdd/foo/apply-progress", project: "marengo" },
      },
    ];

    assert.deepEqual(
      filterRowsByTopicKey(rows, "sdd/foo/tasks", "marengo").map((row) => row.id),
      ["a"],
    );
    assert.deepEqual(
      filterRowsByTopicKey(rows, "sdd/foo/tasks").map((row) => row.id),
      ["a", "b"],
    );
  });
});
