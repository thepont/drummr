import React, { useState } from 'react';

export function EnvelopeEditor({ attack, decay, onChange }: { attack: number, decay: number, onChange: (a: number, d: number) => void }) {
  // Constants for visualization
  const width = 400;
  const height = 200;
  const padding = 20;
  
  // Scaling factors (visual to ms)
  const maxMs = 2000;
  const [activeHandle, setActiveHandle] = useState<'attack' | 'decay' | null>(null);

  const handleMouseDown = (e: React.MouseEvent, type: 'attack' | 'decay') => {
    e.stopPropagation();
    setActiveHandle(type);
    
    const svg = (e.currentTarget as any).ownerSVGElement || e.currentTarget;
    
    const updatePosition = (moveEvent: MouseEvent) => {
      const rect = svg.getBoundingClientRect();
      const x = Math.max(0, Math.min(width, moveEvent.clientX - rect.left));
      const totalMs = (x / width) * maxMs;

      if (type === 'attack') {
        const newAttack = Math.max(1, Math.min(totalMs, 1000));
        onChange(newAttack, decay);
      } else {
        const attackX = (attack / maxMs) * width;
        const newDecay = Math.max(1, totalMs - (attackX / width * maxMs));
        onChange(attack, newDecay);
      }
    };

    const handleMouseUp = () => {
      setActiveHandle(null);
      window.removeEventListener('mousemove', updatePosition);
      window.removeEventListener('mouseup', handleMouseUp);
    };

    window.addEventListener('mousemove', updatePosition);
    window.addEventListener('mouseup', handleMouseUp);
  };

  // Convert ms to visual coords
  const attackX = (attack / maxMs) * width;
  const decayX = ((attack + decay) / maxMs) * width;
  
  const points = `0,${height} ${attackX},${padding} ${decayX},${height}`;

  return (
    <div className="w-full h-full flex flex-col">
      <svg 
        viewBox={`0 0 ${width} ${height}`} 
        className="w-full h-full cursor-default touch-none"
      >
        <defs>
          <linearGradient id="envGradient" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="var(--color-primary)" stopOpacity="0.4" />
            <stop offset="100%" stopColor="var(--color-primary)" stopOpacity="0" />
          </linearGradient>
        </defs>
        
        {/* Grid lines */}
        <line x1="0" y1={height/2} x2={width} y2={height/2} stroke="var(--color-border)" strokeDasharray="4" />
        <line x1={width/2} y1="0" x2={width/2} y2={height} stroke="var(--color-border)" strokeDasharray="4" />

        {/* The Shape */}
        <polyline
          points={points}
          fill="url(#envGradient)"
          stroke="var(--color-primary)"
          strokeWidth="3"
          strokeLinejoin="round"
        />

        {/* Attack handle */}
        <circle 
          cx={attackX} 
          cy={padding} 
          r="8" 
          fill={activeHandle === 'attack' ? "var(--color-primary)" : "var(--color-primary-foreground)"} 
          stroke="var(--color-primary)" 
          strokeWidth="3"
          className="drop-shadow-lg cursor-ew-resize transition-colors"
          onMouseDown={(e) => handleMouseDown(e, 'attack')}
        />

        {/* Decay handle */}
        <circle 
          cx={decayX} 
          cy={height} 
          r="8" 
          fill={activeHandle === 'decay' ? "var(--color-primary)" : "var(--color-primary-foreground)"} 
          stroke="var(--color-primary)" 
          strokeWidth="3"
          className="drop-shadow-lg cursor-ew-resize transition-colors"
          onMouseDown={(e) => handleMouseDown(e, 'decay')}
        />
        
        {/* Labels */}
        <text x={attackX/2} y={height - 10} fontSize="10" fill="var(--color-muted-foreground)" textAnchor="middle">A: {attack.toFixed(0)}ms</text>
        <text x={attackX + decay/2} y={height - 10} fontSize="10" fill="var(--color-muted-foreground)" textAnchor="middle">D: {decay.toFixed(0)}ms</text>
      </svg>
    </div>
  );
}
