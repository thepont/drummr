import { useState, useEffect, useRef } from 'react'
import './App.css'

function App() {
  const [lastMessage, setLastMessage] = useState<string>('Wait...')
  const [midiPort, setMidiPort] = useState<string>('Unknown')
  const [audioDevice, setAudioDevice] = useState<string>('Default')
  const [availablePorts, setAvailablePorts] = useState<string[]>([])
  const [availableAudioDevices, setAvailableAudioDevices] = useState<string[]>([])
  const [isTriggered, setIsTriggered] = useState(false)
  const [wsStatus, setWsStatus] = useState<'Connecting' | 'Connected' | 'Disconnected'>('Disconnected')
  const socketRef = useRef<WebSocket | null>(null)
  const triggerTimeout = useRef<number | null>(null)

  const connect = () => {
    if (socketRef.current) socketRef.current.close()
    
    setWsStatus('Connecting')
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const hostname = window.location.hostname || '127.0.0.1'
    const port = '8080' // Explicitly use backend port
    const socket = new WebSocket(`${protocol}//${hostname}:${port}`)
    socketRef.current = socket

    socket.onopen = () => {
      setWsStatus('Connected')
      setLastMessage('Ready')
      socket.send('LIST_MIDI')
      socket.send('LIST_AUDIO')
    }

    socket.onmessage = (event) => {
      const data = event.data as string
      console.log('WS Received:', data)
      
      if (data.startsWith('PORT: ')) {
        setMidiPort(data.replace('PORT: ', ''))
      } else if (data.startsWith('LIST_MIDI: ')) {
        const ports = data.replace('LIST_MIDI: ', '').split(',')
        setAvailablePorts(ports)
      } else if (data.startsWith('AUDIO_DEVICE: ')) {
        setAudioDevice(data.replace('AUDIO_DEVICE: ', ''))
      } else if (data.startsWith('LIST_AUDIO: ')) {
        const devices = data.replace('LIST_AUDIO: ', '').split(',')
        setAvailableAudioDevices(devices)
      } else if (data.startsWith('MIDI: ')) {
        setLastMessage(data.replace('MIDI: ', ''))
        setIsTriggered(true)
        if (triggerTimeout.current) window.clearTimeout(triggerTimeout.current)
        triggerTimeout.current = window.setTimeout(() => setIsTriggered(false), 100)
      } else {
        setLastMessage(data)
      }
    }

    socket.onclose = () => {
      setWsStatus('Disconnected')
      setLastMessage('Engine offline')
    }

    socket.onerror = (err) => {
      console.error('WS Error:', err)
      setWsStatus('Disconnected')
    }
  }

  useEffect(() => {
    connect()
    return () => socketRef.current?.close()
  }, [])

  const handleMidiChange = (index: number) => {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      socketRef.current.send(`SET_MIDI:${index}`)
    }
  }

  const handleAudioChange = (index: number) => {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      socketRef.current.send(`SET_AUDIO:${index}`)
    }
  }

  return (
    <div className="app-container">
      <header>
        <div className="header-top">
          <h1>drummr <span className="status-badge">POC</span></h1>
          <div className={`ws-indicator ${wsStatus.toLowerCase()}`}>
            {wsStatus} {wsStatus === 'Disconnected' && <button onClick={connect}>Retry</button>}
          </div>
        </div>
        
        <div className="settings-bar">
          <div className="selector">
            <label htmlFor="midi-select">MIDI: </label>
            <select 
              id="midi-select"
              value={availablePorts.indexOf(midiPort)}
              onChange={(e) => handleMidiChange(parseInt(e.target.value))}
            >
              {availablePorts.map((name, index) => (
                <option key={index} value={index}>{name}</option>
              ))}
              {availablePorts.length === 0 && <option>No MIDI devices</option>}
            </select>
          </div>

          <div className="selector">
            <label htmlFor="audio-select">Audio: </label>
            <select 
              id="audio-select"
              value={availableAudioDevices.indexOf(audioDevice)}
              onChange={(e) => handleAudioChange(parseInt(e.target.value))}
            >
              {availableAudioDevices.map((name, index) => (
                <option key={index} value={index}>{name}</option>
              ))}
              {availableAudioDevices.length === 0 && <option>No audio devices</option>}
            </select>
          </div>
        </div>
      </header>
      
      <main>
        <div className={`trigger-pad ${isTriggered ? 'active' : ''}`}>
          <div className="pad-label">MIDI TRIGGER</div>
        </div>

        <div className="info-panel">
          <h3>Last Event</h3>
          <div className="message-display">{lastMessage}</div>
        </div>
      </main>

      <footer>
        <p>Research & Discovery Dashboard | Phase 3</p>
      </footer>
    </div>
  )
}

export default App
