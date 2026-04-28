import { useEffect, useRef, useCallback } from 'react'
import { io, Socket } from 'socket.io-client'
import { useClusterStore } from '../store/cluster'
import type { WsEvent } from '../types'

const WS_URL = import.meta.env.VITE_WS_URL ?? 'http://localhost:8765'

export function useWebSocket() {
  const socketRef = useRef<Socket | null>(null)
  const applyEvent   = useClusterStore(s => s.applyWsEvent)
  const addNotif     = useClusterStore(s => s.addNotification)

  useEffect(() => {
    const socket = io(WS_URL, {
      transports:     ['websocket'],
      reconnection:   true,
      reconnectionDelay: 1000,
      timeout:        5000,
    })
    socketRef.current = socket

    socket.on('connect',    () => addNotif('success', 'Connected', 'Live data stream active'))
    socket.on('disconnect', () => addNotif('warning', 'Disconnected', 'Reconnecting…'))

    socket.on('event', (e: WsEvent) => {
      applyEvent(e)
    })

    return () => { socket.disconnect() }
  }, [])

  const send = useCallback((event: string, data?: unknown) => {
    socketRef.current?.emit(event, data)
  }, [])

  return { send }
}

// ── Mock WebSocket (used in demo/dev without a real backend) ──────────────

export function useMockWebSocket() {
  const applyEvent = useClusterStore(s => s.applyWsEvent)
  const addNotif   = useClusterStore(s => s.addNotification)

  useEffect(() => {
    let running = true
    const vmIds = ['vm-001','vm-002','vm-003','vm-004','vm-005','vm-006','vm-007','vm-008']
    const nodeIds = ['n1','n2','n3']

    // Jitter VM metrics every 2.5s
    const vmTimer = setInterval(() => {
      if (!running) return
      const id = vmIds[Math.floor(Math.random() * vmIds.length)]
      const jitter = () => (Math.random() - 0.5) * 4
      applyEvent({
        type: 'VM_METRICS_UPDATE',
        payload: {
          id,
          cpuUsagePct: Math.max(2, Math.min(95, 40 + jitter() * 5)),
          netRxMbps:   Math.max(0.1, +(5 + Math.random() * 20).toFixed(1)),
          netTxMbps:   Math.max(0.1, +(3 + Math.random() * 10).toFixed(1)),
          memMib:      Math.round(8192 + Math.random() * 1024),
        }
      })
    }, 2500)

    // Node sigma drift every 5s
    const nodeTimer = setInterval(() => {
      if (!running) return
      const id = nodeIds[Math.floor(Math.random() * nodeIds.length)]
      applyEvent({
        type: 'NODE_METRICS_UPDATE',
        payload: {
          id,
          cpuUsagePct: Math.max(5, Math.min(95, 45 + (Math.random() - 0.5) * 20)),
          memUsedMib:  Math.round(150000 + Math.random() * 50000),
          loadScore:   Math.max(0.1, Math.min(0.95, 0.4 + (Math.random() - 0.5) * 0.3)),
        }
      })
    }, 5000)

    // Occasional notifications
    const notifMsgs = [
      ['DRS: migration vm-ml-train-03 → node-03 in progress', 'info'],
      ['Microseg: 847 deny events dev→prod (last 60s)', 'warning'],
      ['node-02 CPU above 70% threshold', 'warning'],
      ['VSAN: replication healthy across all nodes', 'success'],
      ['Migration vm-ml-train-03 complete (blackout: 148ms)', 'success'],
    ] as [string, 'info' | 'warning' | 'success'][]

    let notifIdx = 0
    const notifTimer = setInterval(() => {
      if (!running) return
      const [msg, level] = notifMsgs[notifIdx % notifMsgs.length]
      addNotif(level, 'Caimán OS', msg)
      notifIdx++
    }, 8000)

    // Migration progress updates
    const migTimer = setInterval(() => {
      if (!running) return
      applyEvent({
        type: 'MIGRATION_PROGRESS',
        payload: { vmId: 'vm-008', phase: 'IterativeCopy', progressPct: Math.min(100, 64 + Math.random() * 5) }
      })
    }, 3000)

    return () => {
      running = false
      clearInterval(vmTimer)
      clearInterval(nodeTimer)
      clearInterval(notifTimer)
      clearInterval(migTimer)
    }
  }, [])
}
