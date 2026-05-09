import { render, screen, act } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import App from './App'

let lastWs: MockWebSocket | null = null;

class MockWebSocket {
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: ((err: any) => void) | null = null;
  onmessage: ((event: any) => void) | null = null;
  readyState: number = 0;
  send = vi.fn();
  close = vi.fn();

  constructor(public url: string) {
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

  it('shows connecting status initially', () => {
    render(<App />);
    expect(screen.getAllByText('Connecting')).toHaveLength(1); // Only in desktop sidebar by default
  });

  it('updates status to Connected when socket opens and sends initial requests', () => {
    render(<App />);
    
    act(() => {
      lastWs?.onopen?.();
    });

    expect(screen.getAllByText('Connected')).toHaveLength(1);
    expect(lastWs?.send).toHaveBeenCalledWith('LIST_MIDI');
    expect(lastWs?.send).toHaveBeenCalledWith('LIST_AUDIO');
  });

  it('updates status to Disconnected when socket closes and attempts reconnect', () => {
    render(<App />);
    
    act(() => {
      lastWs?.onopen?.();
    });
    expect(screen.getAllByText('Connected')).toHaveLength(1);

    act(() => {
      lastWs?.onclose?.();
    });
    expect(screen.getAllByText('Disconnected')).toHaveLength(1);

    // Reconnect after 2000ms
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    
    expect(screen.getAllByText('Connecting')).toHaveLength(1);
  });
});

describe('App Message Parsing', () => {
  beforeEach(() => {
    vi.stubGlobal('WebSocket', MockWebSocket);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('updates MIDI port when receiving PORT message', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'PORT: My MIDI Port' });
    });

    // One in sidebar, one in Dashboard Card
    expect(screen.getAllByText('My MIDI Port')).toHaveLength(2);
  });

  it('updates Audio device when receiving AUDIO_DEVICE message', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'AUDIO_DEVICE: High Fidelity Output' });
    });

    // One in sidebar, one in Dashboard Card
    expect(screen.getAllByText('High Fidelity Output')).toHaveLength(2);
  });

  it('updates available MIDI ports when receiving LIST_MIDI message', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'LIST_MIDI: Port A,Port B' });
    });

    expect(screen.getByText('Port A')).toBeInTheDocument();
    expect(screen.getByText('Port B')).toBeInTheDocument();
  });

  it('updates available Audio devices when receiving LIST_AUDIO message', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'LIST_AUDIO: Output 1,Output 2' });
    });

    expect(screen.getByText('Output 1')).toBeInTheDocument();
    expect(screen.getByText('Output 2')).toBeInTheDocument();
  });

  it('updates last MIDI note when receiving MIDI message', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'MIDI: 60,100' });
    });

    expect(screen.getByText('Note 60 (Vel 100)')).toBeInTheDocument();
  });

  it('does not update last MIDI note when velocity is 0', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      // First a real note
      lastWs?.onmessage?.({ data: 'MIDI: 60,100' });
    });
    expect(screen.getByText('Note 60 (Vel 100)')).toBeInTheDocument();

    act(() => {
      // Then note off
      lastWs?.onmessage?.({ data: 'MIDI: 60,0' });
    });
    // Should still show the last active note
    expect(screen.getByText('Note 60 (Vel 100)')).toBeInTheDocument();
  });

  it('handles malformed MIDI messages gracefully', () => {
    const consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'MIDI: invalid,data' });
    });
    
    expect(screen.getByText('No Input')).toBeInTheDocument();
    consoleSpy.mockRestore();
  });
});

describe('App User Interactions', () => {
  beforeEach(() => {
    vi.stubGlobal('WebSocket', MockWebSocket);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('sends LIST_MIDI when Refresh MIDI is clicked', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
    });
    
    const refreshButton = screen.getAllByRole('button', { name: /refresh/i })[0];
    act(() => {
      refreshButton.click();
    });
    
    expect(lastWs?.send).toHaveBeenCalledWith('LIST_MIDI');
  });

  it('sends SELECT_MIDI when a MIDI port is clicked', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'LIST_MIDI: Port A,Port B' });
    });
    
    const portBButton = screen.getByText('Port B');
    act(() => {
      portBButton.click();
    });
    
    expect(lastWs?.send).toHaveBeenCalledWith('SELECT_MIDI:1');
  });

  it('sends SELECT_AUDIO when an audio device is clicked', () => {
    render(<App />);
    act(() => {
      lastWs?.onopen?.();
      lastWs?.onmessage?.({ data: 'LIST_AUDIO: Audio 1,Audio 2' });
    });
    
    const audio2Button = screen.getByText('Audio 2');
    act(() => {
      audio2Button.click();
    });
    
    expect(lastWs?.send).toHaveBeenCalledWith('SELECT_AUDIO:1');
  });
});
