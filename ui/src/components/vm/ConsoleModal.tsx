import React, { useEffect, useRef, useState, useCallback } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Terminal as XTerm }   from '@xterm/xterm'
import { FitAddon }            from '@xterm/addon-fit'
import { WebLinksAddon }       from '@xterm/addon-web-links'
import {
  X, Maximize2, Minimize2, Copy, Trash2,
  Download, Wifi, WifiOff, RefreshCw, Terminal,
} from 'lucide-react'
import '@xterm/xterm/css/xterm.css'

interface Props {
  vmId:   string
  vmName: string
  onClose: () => void
}

type ConnState = 'connecting' | 'connected' | 'disconnected' | 'reconnecting'

const WS_BASE = (() => {
  const api = import.meta.env.VITE_API_URL ?? 'http://localhost:8765'
  return api.replace(/^http/, 'ws')
})()

const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 15000]

export function ConsoleModal({ vmId, vmName, onClose }: Props) {
  const termRef    = useRef<HTMLDivElement>(null)
  const xtermRef   = useRef<XTerm | null>(null)
  const fitRef     = useRef<FitAddon | null>(null)
  const wsRef      = useRef<WebSocket | null>(null)
  const retryRef   = useRef(0)
  const logRef     = useRef<string[]>([])
  const pingRef    = useRef<ReturnType<typeof setInterval> | null>(null)
  const pingTimeRef = useRef<number>(0)

  const [connState, setConnState] = useState<ConnState>('connecting')
  const [latency,   setLatency]   = useState<number | null>(null)
  const [dims,      setDims]      = useState({ cols: 80, rows: 24 })
  const [fullscreen, setFullscreen] = useState(false)
  const [copied,    setCopied]    = useState(false)

  // ── xterm init ──────────────────────────────────────────────────────────
  useEffect(() => {
    if (!termRef.current) return

    const term = new XTerm({
      fontFamily: '"IBM Plex Mono", "Fira Code", monospace',
      fontSize:   13,
      lineHeight: 1.35,
      letterSpacing: 0.5,
      theme: {
        background:    '#020d02',
        foreground:    '#c8e6c9',
        cursor:        '#69f0ae',
        cursorAccent:  '#020d02',
        selectionBackground: 'rgba(105,240,174,0.25)',
        black:   '#021202', red:     '#ef5350', green:   '#69f0ae',
        yellow:  '#ffca28', blue:    '#42a5f5', magenta: '#ce93d8',
        cyan:    '#26c6da', white:   '#c8e6c9',
        brightBlack:   '#1b5e20', brightRed:     '#ff5252',
        brightGreen:   '#b9f6ca', brightYellow:  '#ffe57f',
        brightBlue:    '#82b1ff', brightMagenta: '#ea80fc',
        brightCyan:    '#84ffff', brightWhite:   '#f1f8e9',
      },
      scrollback:      5000,
      cursorBlink:     true,
      cursorStyle:     'block',
      allowProposedApi: true,
      convertEol:      false,
    })

    const fit   = new FitAddon()
    const links = new WebLinksAddon()
    term.loadAddon(fit)
    term.loadAddon(links)
    term.open(termRef.current)
    fit.fit()
    setDims({ cols: term.cols, rows: term.rows })

    xtermRef.current = term
    fitRef.current   = fit

    // Filter out terminal response sequences that xterm.js sends automatically
    // (cursor position reports, device attributes) - these confuse the guest shell.
    // Pattern: ESC [ ... R  (cursor position)  |  ESC [ ? ... c  (DA)
    const ANSI_RESPONSE = /\x1b\[[\d;?]*[Rc]/g

    // Keyboard -> WS
    term.onData(data => {
      if (wsRef.current?.readyState !== WebSocket.OPEN) return
      const clean = data.replace(ANSI_RESPONSE, '')
      if (clean) wsRef.current.send(clean)
    })

    // Resize observer
    const ro = new ResizeObserver(() => {
      fit.fit()
      setDims({ cols: term.cols, rows: term.rows })
    })
    ro.observe(termRef.current)

    return () => {
      ro.disconnect()
      term.dispose()
      xtermRef.current = null
    }
  }, [])

  // ── WebSocket ───────────────────────────────────────────────────────────
  const connect = useCallback(() => {
    const token = localStorage.getItem('caiman_token') ?? ''
    const url = `${WS_BASE}/api/vms/${vmId}/console/ws${token ? `?token=${token}` : ''}`
    const ws  = new WebSocket(url)
    ws.binaryType = 'arraybuffer'
    wsRef.current = ws
    setConnState('connecting')

    ws.onopen = () => {
      setConnState('connected')
      retryRef.current = 0
      xtermRef.current?.write('\r\x1b[32m[caiman] \x1b[2mconnected\x1b[0m\r\n')

      // No application-level ping; the browser sends WS Ping frames automatically
    }

    ws.onmessage = (ev) => {
      const term = xtermRef.current
      if (!term) return

      let text: string
      if (ev.data instanceof ArrayBuffer) {
        text = new TextDecoder().decode(ev.data)
      } else {
        text = ev.data as string
      }

      // Log accumulation for download
      logRef.current.push(text)
      if (logRef.current.length > 10000) logRef.current.shift()

      term.write(text)
    }

    ws.onclose = () => {
      if (pingRef.current) clearInterval(pingRef.current)
      setConnState('disconnected')
      const delay = RECONNECT_DELAYS[Math.min(retryRef.current, RECONNECT_DELAYS.length - 1)]
      retryRef.current++
      xtermRef.current?.write(`\r\x1b[33m[caiman] \x1b[2mdisconnected — reconnecting in ${delay/1000}s...\x1b[0m\r\n`)
      setConnState('reconnecting')
      setTimeout(connect, delay)
    }

    ws.onerror = () => {
      xtermRef.current?.write('\r\x1b[31m[caiman] \x1b[2mconnection error\x1b[0m\r\n')
    }
  }, [vmId])

  useEffect(() => {
    // Wait for xterm to init
    const t = setTimeout(connect, 100)
    return () => {
      clearTimeout(t)
      if (pingRef.current) clearInterval(pingRef.current)
      wsRef.current?.close()
    }
  }, [connect])

  // ── Fullscreen ──────────────────────────────────────────────────────────
  useEffect(() => {
    setTimeout(() => fitRef.current?.fit(), 50)
  }, [fullscreen])

  // ── Actions ─────────────────────────────────────────────────────────────
  const handleCopy = () => {
    const sel = xtermRef.current?.getSelection()
    if (sel) {
      navigator.clipboard.writeText(sel)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    }
  }

  const handleClear = () => xtermRef.current?.clear()

  const handleDownload = () => {
    const blob = new Blob([logRef.current.join('')], { type: 'text/plain' })
    const a    = document.createElement('a')
    a.href     = URL.createObjectURL(blob)
    a.download = `${vmId}-console.log`
    a.click()
    URL.revokeObjectURL(a.href)
  }

  const handleReconnect = () => {
    wsRef.current?.close()
    retryRef.current = 0
    setTimeout(connect, 200)
  }

  // ── Keyboard shortcuts ──────────────────────────────────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !fullscreen) onClose()
      if (e.key === 'F11') { e.preventDefault(); setFullscreen(f => !f) }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [fullscreen, onClose])

  // ── Render ───────────────────────────────────────────────────────────────
  const connColor = {
    connecting:    'text-caiman-amber',
    connected:     'text-caiman-green',
    disconnected:  'text-caiman-red',
    reconnecting:  'text-caiman-amber',
  }[connState]

  const connIcon = connState === 'connected'
    ? <Wifi size={10} />
    : connState === 'connecting' || connState === 'reconnecting'
      ? <RefreshCw size={10} className="animate-spin" />
      : <WifiOff size={10} />

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className={`fixed inset-0 z-50 flex items-center justify-center
                    ${fullscreen ? '' : 'bg-black/70 backdrop-blur-sm'}`}
        onClick={e => { if (e.target === e.currentTarget && !fullscreen) onClose() }}
      >
        <motion.div
          initial={{ scale: 0.95, opacity: 0 }}
          animate={{ scale: 1,    opacity: 1 }}
          exit={{    scale: 0.95, opacity: 0 }}
          transition={{ type: 'spring', stiffness: 400, damping: 35 }}
          className={`flex flex-col bg-[#020d02] border border-[#1b5e20]
                      shadow-[0_0_60px_rgba(27,94,32,0.4)]
                      ${fullscreen
                        ? 'fixed inset-0 rounded-none'
                        : 'rounded-lg w-[900px] h-[600px] max-w-[95vw] max-h-[90vh]'
                      }`}
        >
          {/* ── Title bar ── */}
          <div className="flex items-center gap-2 px-3 py-2
                          border-b border-[#1b5e20]/60 flex-shrink-0
                          bg-[#030f03] rounded-t-lg">

            {/* Traffic lights */}
            <div className="flex gap-1.5 mr-1">
              <button onClick={onClose}
                className="w-3 h-3 rounded-full bg-[#ef5350] hover:bg-[#ff5252]
                           transition-colors flex items-center justify-center group">
                <X size={6} className="opacity-0 group-hover:opacity-100 text-[#7f0000]" />
              </button>
              <div className="w-3 h-3 rounded-full bg-[#ffca28]/40" />
              <button onClick={() => setFullscreen(f => !f)}
                className="w-3 h-3 rounded-full bg-[#69f0ae] hover:bg-[#b9f6ca]
                           transition-colors flex items-center justify-center group">
                <Maximize2 size={6} className="opacity-0 group-hover:opacity-100 text-[#004d40]" />
              </button>
            </div>

            {/* VM name */}
            <Terminal size={11} className="text-caiman-green" />
            <span className="text-[11px] font-mono text-[#c8e6c9] flex-1 truncate">
              {vmName}
              <span className="text-caiman-dim ml-2 text-[9px]">· {vmId}</span>
            </span>

            {/* Dims */}
            <span className="text-[8px] font-mono text-caiman-dim">
              {dims.cols}×{dims.rows}
            </span>

            {/* Latency */}
            {latency !== null && (
              <span className="text-[8px] font-mono text-caiman-dim">
                {latency}ms
              </span>
            )}

            {/* Connection state */}
            <span className={`flex items-center gap-1 text-[8px] font-mono uppercase
                              tracking-[1px] ${connColor}`}>
              {connIcon}
              {connState}
            </span>

            {/* Actions */}
            <div className="flex items-center gap-0.5 ml-1">
              <IconBtn title="Reconnect (if disconnected)"  onClick={handleReconnect}>
                <RefreshCw size={11} />
              </IconBtn>
              <IconBtn title={copied ? 'Copied!' : 'Copy selection'} onClick={handleCopy}>
                <Copy size={11} className={copied ? 'text-caiman-green' : ''} />
              </IconBtn>
              <IconBtn title="Clear terminal" onClick={handleClear}>
                <Trash2 size={11} />
              </IconBtn>
              <IconBtn title="Download log" onClick={handleDownload}>
                <Download size={11} />
              </IconBtn>
              <IconBtn title={fullscreen ? 'Exit fullscreen (F11)' : 'Fullscreen (F11)'}
                       onClick={() => setFullscreen(f => !f)}>
                {fullscreen ? <Minimize2 size={11} /> : <Maximize2 size={11} />}
              </IconBtn>
              <IconBtn title="Close (Esc)" onClick={onClose}>
                <X size={11} />
              </IconBtn>
            </div>
          </div>

          {/* ── Terminal body ── */}
          <div className="flex-1 relative overflow-hidden">
            {/* CRT scanlines */}
            <div className="absolute inset-0 pointer-events-none z-10"
                 style={{
                   background: 'repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(0,0,0,0.03) 2px, rgba(0,0,0,0.03) 4px)',
                 }} />
            {/* Vignette */}
            <div className="absolute inset-0 pointer-events-none z-10"
                 style={{
                   background: 'radial-gradient(ellipse at center, transparent 60%, rgba(0,0,0,0.4) 100%)',
                 }} />
            <div ref={termRef} className="w-full h-full p-2" />
          </div>

          {/* ── Status bar ── */}
          <div className="flex items-center gap-3 px-3 py-1.5
                          border-t border-[#1b5e20]/40 flex-shrink-0
                          bg-[#030f03] rounded-b-lg
                          text-[8px] font-mono text-caiman-dim tracking-[0.5px]">
            <span className={connColor}>{connState.toUpperCase()}</span>
            <span>·</span>
            <span>{vmId}</span>
            <span>·</span>
            <span>ttyS0 115200n8</span>
            {latency !== null && <><span>·</span><span>{latency}ms RTT</span></>}
            <span className="ml-auto">ESC close · F11 fullscreen · ctrl+shift+C copy</span>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  )
}

function IconBtn({ children, onClick, title }: {
  children: React.ReactNode
  onClick: () => void
  title?: string
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className="p-1 text-caiman-dim hover:text-caiman-bright
                 hover:bg-[#1b5e20]/30 rounded transition-colors"
    >
      {children}
    </button>
  )
}
