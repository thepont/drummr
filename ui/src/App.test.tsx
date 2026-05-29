import { render, screen, act } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import App from './App'

let lastWs: MockWebSocket | null = null;

class MockWebSocket {
  url: string;
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: ((err: any) => void) | null = null;
  onmessage: ((event: any) => void) | null = null;
  readyState: number = 0;
  send = vi.fn();
  close = vi.fn();

  constructor(url: string) {
    this.url = url;
    lastWs = this;
  }
}

describe('App WebSocket Lifecycle', () => {
  beforeEach(() => {
    vi.stubGlobal('WebSocket', MockWebSocket);
    vi.useFakeTimers();
    lastWs = null;
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('renders and attempts connection', () => {
    render(<App />);
    expect(screen.getByText(/Connecting/i)).toBeDefined();
  });

  it('updates connection state on open', async () => {
    render(<App />);
    
    await act(async () => {
      if (lastWs?.onopen) lastWs.onopen();
    });

    expect(screen.getAllByText(/Connected/i)[0]).toBeDefined();
  });

  it('reconnects when connection lost', async () => {
    render(<App />);
    
    await act(async () => {
      if (lastWs?.onopen) lastWs.onopen();
    });

    const firstWs = lastWs;
    
    await act(async () => {
      if (firstWs?.onclose) firstWs.onclose();
      vi.advanceTimersByTime(2000);
    });

    expect(lastWs).not.toBe(firstWs);
  });
});
