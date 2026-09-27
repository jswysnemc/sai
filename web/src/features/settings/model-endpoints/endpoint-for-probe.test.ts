import { describe, expect, it } from "vitest";
import type { ModelEndpointConfig } from "../../../api/contracts";
import { endpointForProbe } from "./endpoint-for-probe";

const endpoint = {
  id: "image",
  kind: "image_generation",
  name: "Image",
  endpoint: "https://images.example/generate",
  api_key: "",
  model: "gpt-image-1"
} as ModelEndpointConfig;

describe("endpointForProbe", () => {
  it("keeps a single saved key on the legacy field so the server can restore it", () => {
    const draft = endpointForProbe(
      endpoint,
      [{ id: "key-1", api_key: "SAVED_SECRET", label: "" }],
      "key-1",
      "SAVED_SECRET"
    );
    expect(draft.api_key).toBe("SAVED_SECRET");
    expect(draft.api_keys).toEqual([]);
    expect(draft.api_key_balance).toBe(false);
  });

  it("sends only the selected key when the editor holds several", () => {
    const draft = endpointForProbe(
      endpoint,
      [
        { id: "key-1", api_key: "one", label: "" },
        { id: "key-2", api_key: "two", label: "" }
      ],
      "key-2",
      "SAVED_SECRET"
    );
    expect(draft.api_keys).toEqual([{ id: "key-2", api_key: "two", label: "" }]);
    expect(draft.api_key_selected).toBe("key-2");
    expect(draft.api_key).toBe("");
  });
});
