# Conversational SVG editing

Updated 2026-11-12. The Glory/Sapid desktop and Salvo API share `sivgo-agent`.
The provider protocol is OpenAI-compatible `POST {base_url}/chat/completions`.
Streaming and tool calling are not implemented.

## Desktop

1. Open **AI Agent** and use its settings icon to configure the API base URL,
   model ID and optional key. Settings and keys stay in session memory.
   Environment defaults: `SIVGO_AI_BASE_URL`, `SIVGO_AI_MODEL`, `SIVGO_AI_API_KEY`.
2. Select objects when referring to a selection. Objects without XML IDs receive
   temporary labels in the request snapshot; sending never changes the drawing.
3. Send an instruction. The provider receives the current SVG and recent chat.
   **Stop AI request** cancels the HTTP connection and restores the instruction.
4. Review the preview and attribute changes, then apply or discard the whole
   transaction. Applying creates one undo step.

Object edits preserve native node identities, selection and untouched nodes.
Full drawing replacements have an explicit review label and can replace object
identities. A proposal cannot apply if the source drawing has changed. Closing
the panel keeps the chat; changing drawings or starting a new chat cancels work.

## Provider reply

For attribute changes, prefer:

```json
{
  "message": "Changed the rectangle to blue.",
  "svg": null,
  "operations": [
    { "id": "rectangle-1", "attributes": { "fill": "#2563eb", "opacity": "0.8", "stroke": null } }
  ]
}
```

Strings set attributes; null removes them. Geometry, `d`, `transform`, `style`,
font and paint attributes are supported within the validated static SVG subset.
Existing CSS may override a presentation attribute; the model must account for
the cascade. Each target must be a unique existing XML ID, occurring once per
transaction. Changing IDs, namespaces, qualified attributes, text content or
document structure requires a full SVG proposal:

```json
{ "message": "Added a circle.", "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"20\"/></svg>" }
```

Return either a full SVG or operations, never both. To answer without editing,
return `message` with `"svg": null` and omit operations.

## Server API

Enable `SIVGO_AI_ENABLE_API=1` and the provider environment variables. Send an
authenticated `POST /api/v1/agent/suggest` request:

```json
{
  "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect id=\"rectangle-1\" width=\"20\" height=\"20\"/></svg>",
  "prompt": "Make the selected rectangle blue",
  "selected_ids": ["rectangle-1"],
  "history": []
}
```

Responses include `message` and nullable `svg`. Object edits also include
`operations`, already resolved into the validated SVG. Apply the SVG once, or
apply operations to the exact request snapshot to preserve native identities.
Do not apply both. The endpoint never writes to the document store.
`/api/v1/openapi.json` describes the schema.

Limits: SVG 256 KiB, prompt/message 4 KiB, 12 history messages, 100 selected IDs,
100 edited objects, 32 attributes/object, 16 KiB/attribute value, and 2 MiB/provider
response. Reviews show up to 100 attribute rows and a remaining-change count;
approval applies all changes. Server calls share 10 requests/minute and two
concurrent calls. Remote URLs require HTTPS; loopback HTTP supports local models.
Redirects are disabled. Unsafe or ambiguous edits reject the whole transaction.

## Verification and follow-up

Mock-provider regressions cover credentials, selection context, multi-turn chat,
full SVG previews, object changes, retained IDs/selection, undo/redo, and actual
in-flight HTTP cancellation. Paid-provider/model compatibility requires a
separate integration run with that provider.

Remaining: streaming, model discovery, connection diagnostics, OS credential
storage, structured creation/removal, approval per operation, and context
selection for larger documents.
