import { render, screen, fireEvent } from '@testing-library/react'
import { describe, it, expect, vi } from 'vitest'
import { ParamSlider } from './ui'

describe('ParamSlider', () => {
  it('renders with label and value', () => {
    render(
      <ParamSlider 
        label="Freq" 
        value={440} 
        min={20} 
        max={2000} 
        step={1} 
        onChange={() => {}} 
      />
    )
    expect(screen.getByText('Freq')).toBeInTheDocument()
    expect(screen.getByText('440.00')).toBeInTheDocument()
  })

  it('renders modulation slots', () => {
    const mods = [
      { source: 'Envelope', depth: 0.5 },
      { source: 'Lfo1', depth: -0.2 }
    ]
    render(
      <ParamSlider 
        label="Freq" 
        value={440} 
        min={20} 
        max={2000} 
        step={1} 
        mods={mods}
        onChange={() => {}} 
        onModChange={() => {}}
      />
    )
    // Assuming we show the source names
    expect(screen.getByText('Env')).toBeInTheDocument()
    expect(screen.getByText('LFO 1')).toBeInTheDocument()
  })

  it('calls onModChange when depth changes', () => {
    const onModChange = vi.fn()
    const mods = [{ source: 'Envelope', depth: 0.5 }]
    render(
      <ParamSlider 
        label="Freq" 
        value={440} 
        min={20} 
        max={2000} 
        step={1} 
        mods={mods}
        onChange={() => {}} 
        onModChange={onModChange}
      />
    )
    
    // Find depth slider for mod slot. 
    // This is speculative until I design the UI, but let's assume it has an aria-label or similar.
    const depthSlider = screen.getByLabelText(/depth/i)
    fireEvent.change(depthSlider, { target: { value: '0.8' } })
    
    expect(onModChange).toHaveBeenCalledWith(0, 'Envelope', 0.8)
  })
})
