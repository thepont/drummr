import { render, screen, act, fireEvent } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import MappingView from './MappingView'

// Mock icons
vi.mock('@phosphor-icons/react', () => ({
  Plus: () => <div data-testid="icon-plus" />,
  List: () => <div data-testid="icon-list" />,
  Target: () => <div data-testid="icon-target" />,
  MagnifyingGlass: () => <div data-testid="icon-search" />,
  Trash: () => <div data-testid="icon-trash" />,
  FloppyDisk: () => <div data-testid="icon-save" />,
}))

describe('MappingView', () => {
  let mockWs: any;
  let messageHandler: (ev: any) => void;

  beforeEach(() => {
    vi.useFakeTimers();
    mockWs = {
      send: vi.fn(),
      addEventListener: vi.fn((event, handler) => {
        if (event === 'message') messageHandler = handler;
      }),
      removeEventListener: vi.fn(),
      readyState: 1, // WebSocket.OPEN
    };
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const initialMapping = [
    { slot: 0, name: 'Kick Drum', note: 36 },
    { slot: 1, name: 'Snare Drum', note: 38 },
  ];

  it('sends GET_MAPPING on mount and handles response', async () => {
    render(<MappingView ws={mockWs} />);

    expect(mockWs.send).toHaveBeenCalledWith('GET_MAPPING');

    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    expect(screen.getAllByText('Kick Drum').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Snare Drum').length).toBeGreaterThan(0);
    expect(screen.getByText('Note 36')).toBeInTheDocument();
    expect(screen.getByText('Note 38')).toBeInTheDocument();
  });

  it('activates learning mode when a pad is clicked', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    // Find the Kick Drum pad specifically in the grid
    const pads = screen.getAllByRole('button');
    const kickPad = pads.find(p => p.textContent?.includes('Slot 0') && p.textContent?.includes('Kick Drum'));
    fireEvent.click(kickPad!);

    expect(screen.getByText(/Learning Mode Active/i)).toBeInTheDocument();
    expect(screen.getByText(/Hit a physical pad to assign it to/i)).toBeInTheDocument();
    
    // Check if the Kick Drum text in the pad is amber (learning mode)
    const kickText = screen.getAllByText('Kick Drum').find(el => el.tagName === 'SPAN' && el.classList.contains('text-amber-500'));
    expect(kickText).toBeDefined();
  });

  it('updates mapping when MIDI message is received in learning mode', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    // Enter learning mode for Snare (slot 1)
    const pads = screen.getAllByRole('button');
    const snarePad = pads.find(p => p.textContent?.includes('Slot 1') && p.textContent?.includes('Snare Drum'));
    fireEvent.click(snarePad!);

    // Receive MIDI message for note 40
    await act(async () => {
      messageHandler({ data: 'MIDI: 40,100' });
    });

    expect(mockWs.send).toHaveBeenCalledWith('UPDATE_MAPPING:1:40');
    expect(screen.queryByText(/Learning Mode Active/i)).not.toBeInTheDocument();
    expect(screen.getByText('Note 40')).toBeInTheDocument();
  });

  it('shows active state when MIDI message is received', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    await act(async () => {
      messageHandler({ data: 'MIDI: 36,100' });
    });

    // Check if Kick Drum Pad (note 36) has active class (bg-primary)
    const pads = screen.getAllByRole('button');
    const kickPad = pads.find(p => p.textContent?.includes('Slot 0') && p.textContent?.includes('Kick Drum'));
    expect(kickPad).toHaveClass('bg-primary');

    // Fast-forward 100ms
    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(kickPad).not.toHaveClass('bg-primary');
  });

  it('filters roles based on search query', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    const searchInput = screen.getByPlaceholderText('Search roles...');
    fireEvent.change(searchInput, { target: { value: 'Kick' } });

    // The input in Role List should still show 'Kick Drum'
    expect(screen.getByDisplayValue('Kick Drum')).toBeInTheDocument();
    // But 'Snare Drum' input should be gone
    expect(screen.queryByDisplayValue('Snare Drum')).not.toBeInTheDocument();
  });

  it('can delete a role', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    const deleteButtons = screen.getAllByTestId('icon-trash');
    fireEvent.click(deleteButtons[0].parentElement!);

    expect(screen.queryByDisplayValue('Kick Drum')).not.toBeInTheDocument();
    expect(screen.getByText('Save Changes')).toBeInTheDocument();
  });

  it('sends SAVE_MAPPING when Save Changes is clicked', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    // Make a change to show Save button
    const deleteButtons = screen.getAllByTestId('icon-trash');
    fireEvent.click(deleteButtons[0].parentElement!);

    const saveButton = screen.getByText('Save Changes');
    fireEvent.click(saveButton);

    expect(mockWs.send).toHaveBeenCalledWith(expect.stringMatching(/SAVE_MAPPING:/));
    expect(screen.getByText('Saving...')).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(screen.queryByText('Saving...')).not.toBeInTheDocument();
  });

  it('adds a preset role', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: `MAPPING: ${JSON.stringify(initialMapping)}` });
    });

    const addPresetButton = screen.getByText('Add Preset Role');
    fireEvent.click(addPresetButton);

    // Find "Hi-Hat" in the preset list dropdown specifically
    const presetOptions = screen.getAllByText('Hi-Hat');
    const hiHatOption = presetOptions.find(opt => opt.tagName === 'BUTTON' && opt.classList.contains('w-full'));
    fireEvent.click(hiHatOption!);

    expect(screen.getByDisplayValue('Hi-Hat')).toBeInTheDocument();
    expect(screen.getByText(/Learning Mode Active/i)).toBeInTheDocument();
    
    // Check learning message specifically
    const learningMsg = screen.getByText(/Hit a physical pad to assign it to/i).parentElement;
    expect(learningMsg?.textContent).toContain('Hi-Hat');
  });

  it('re-requests mapping when KIT message is received', async () => {
    render(<MappingView ws={mockWs} />);
    
    await act(async () => {
      messageHandler({ data: 'KIT: some_kit_data' });
    });

    expect(mockWs.send).toHaveBeenCalledWith('GET_MAPPING');
  });
});
