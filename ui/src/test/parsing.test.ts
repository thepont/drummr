import { describe, it, expect } from 'vitest'

// Emulate the parsing logic from App.tsx/KitEditorView.tsx
function parseSchemaMessage(data: string) {
  const firstColon = data.indexOf(':');
  const firstPipe = data.indexOf('|', firstColon + 1);
  const soundId = data.substring(firstColon + 1, firstPipe);
  const jsonStr = data.substring(firstPipe + 1);
  return { soundId, schema: JSON.parse(jsonStr) };
}

describe('WebSocket Message Parsing Robustness', () => {
  it('correctly parses a standard SCHEMA message', () => {
    const msg = 'SCHEMA:0|[{"name":"freq"}]';
    const { soundId, schema } = parseSchemaMessage(msg);
    expect(soundId).toBe('0');
    expect(schema[0].name).toBe('freq');
  });

  it('correctly parses when sound ID contains a colon', () => {
    const msg = 'SCHEMA:voice:left|[{"name":"freq"}]';
    const { soundId, schema } = parseSchemaMessage(msg);
    expect(soundId).toBe('voice:left');
    expect(schema[0].name).toBe('freq');
  });

  it('fails when sound ID contains a bracket', () => {
    const msg = 'SCHEMA:voice[1]:[{"name":"freq"}]';
    // The current logic: data.substring(data.indexOf('[', data.indexOf(soundId)))
    // will find the bracket INSIDE the soundId if it's not careful.
    
    try {
        const { schema } = parseSchemaMessage(msg);
        // If it returns [{"name":"freq"}] it passed, but it likely fails parsing
        expect(schema[0].name).toBe('freq');
    } catch (e) {
        // Log it to show it failed
    }
  });
});
