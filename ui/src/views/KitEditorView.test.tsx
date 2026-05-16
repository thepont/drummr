import { render, screen, act } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import KitEditorView from './KitEditorView'

// Mock the Button, Slider, Card components from ui.tsx if necessary or just render them
vi.mock('../components/ui', () => ({
  cn: (...args: any[]) => args.filter(Boolean).join(' '),
  Slider: ({ label, value }: any) => <div data-testid="slider">{label}: {value}</div>,
  ParamController: ({ label, value }: any) => <div data-testid="slider">{label}: {value}</div>,
  FrequencyVisualizer: ({ value }: any) => <div data-testid="slider">Base Pitch: {value}</div>,
  PredictiveGraph: () => <div data-testid="predictive-graph" />,
  Button: ({ children, onClick }: any) => <button onClick={onClick}>{children}</button>,
  Card: ({ title, value }: any) => <div>{title}: {value}</div>,
}))

describe('KitEditorView Schema Parsing', () => {
  let mockWs: any;

  beforeEach(() => {
    mockWs = {
      send: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    };
  });

  it('correctly parses a complex SCHEMA message with colons', async () => {
    let messageHandler: any;
    mockWs.addEventListener.mockImplementation((event: string, handler: any) => {
      if (event === 'message') messageHandler = handler;
    });

    const mockSetSounds = vi.fn();
    const mockSetSchemas = vi.fn();
    const mockSetSelectedId = vi.fn();

    render(<KitEditorView 
      ws={mockWs} 
      sounds={[]} 
      setSounds={mockSetSounds}
      schemas={{}}
      setSchemas={mockSetSchemas}
      selectedSoundId={null}
      setSelectedSoundId={mockSetSelectedId}
    />);

    // Mock initial kit load
    const kitData = [
      { id: '0', name: 'Laser Kick', engine_type: 'fm', freq: 55, attack: 1, decay: 400, mod_ratio: 1, mod_index: 6, noise_level: 0 }
    ];
    
    await act(async () => {
      messageHandler({ data: `KIT: ${JSON.stringify(kitData)}` });
    });

    // Mock SCHEMA message with colons in JSON
    const schemaData = [
      { name: 'freq', min: 20, max: 2000, default: 440, unit: 'Hz' },
      { name: 'mod_ratio', min: 0, max: 10, default: 1, unit: 'ratio' }
    ];
    
    await act(async () => {
      messageHandler({ data: `SCHEMA:0:${JSON.stringify(schemaData)}` });
    });

    expect(screen.getByText(/Base Pitch: 55/)).toBeInTheDocument();
    expect(screen.getByText(/Mod ratio: 1/)).toBeInTheDocument();
  });
});
