import { useState, useEffect, useRef } from 'react'
import './App.css'

function App() {
  const [lastMessage, setLastMessage] = useState<string>('Waiting for MIDI...')
  const [isTriggered, setIsTriggered] = useState(false)
  const triggerTimeout = useRef<number | null>(null)

  useEffect(() => {
    const socket = new WebSocket('ws://127.0.0.1:8080')

    socket.onopen = () => {
      setLastMessage('Connected to drummr engine')
    }

    socket.onmessage = (event) => {
      setLastMessage(event.data)
      
      // Flash the trigger indicator
      setIsTriggered(true)
      if (triggerTimeout.current) window.clearTimeout(triggerTimeout.current)
      triggerTimeout.current = window.setTimeout(() => setIsTriggered(false), 100)
    }

    socket.onclose = () => {
      setLastMessage('Disconnected from engine')
    }

    return () => socket.close()
  }, [])

  return (
    <div className="app-container">
      <header>
        <h1>drummr <span className="status-badge">POC</span></h1>
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
