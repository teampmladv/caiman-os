import React from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { X } from 'lucide-react'
import { useClusterStore } from '../../store/cluster'

// ── ActivityFeed ──────────────────────────────────────────────────────────

export function ActivityFeed() {
  const notifs = useClusterStore(s => s.notifications.slice(0, 8))
  const levelColor: Record<string, string> = {
    info:    'text-caiman-blue',
    success: 'text-caiman-bright',
    warning: 'text-caiman-amber',
    error:   'text-caiman-red',
  }
  const icon = (level: string) => {
    if (level === 'success') return '✓'
    if (level === 'warning') return '⚠'
    if (level === 'error')   return '✕'
    return 'ℹ'
  }
  return (
    <div className="panel flex-1">
      <div className="panel-hd">
        <span className="panel-title">Activity</span>
      </div>
      <div className="panel-body">
        {notifs.map(n => (
          <div key={n.id} className="flex gap-2 py-1 border-b border-caiman-bg last:border-b-0">
            <span className={`text-[9px] ${levelColor[n.level] ?? 'text-caiman-text'} flex-shrink-0`}>
              {icon(n.level)}
            </span>
            <span className="text-[9px] text-caiman-text leading-relaxed">{n.message}</span>
          </div>
        ))}
        {notifs.length === 0 && (
          <div className="text-[9px] text-caiman-dim py-2 text-center">No recent activity</div>
        )}
      </div>
    </div>
  )
}

// ── NotificationStack ─────────────────────────────────────────────────────

export function NotificationStack() {
  const { notifications, clearNotif } = useClusterStore(s => ({
    notifications: s.notifications.filter(n => !n.read).slice(0, 4),
    clearNotif:    s.clearNotif,
  }))

  const levelBorder: Record<string, string> = {
    info:    'border-caiman-blue',
    success: 'border-caiman-border2',
    warning: 'border-caiman-amber',
    error:   'border-caiman-red',
  }

  return (
    <div className="fixed bottom-10 right-3 flex flex-col gap-1.5 z-50 pointer-events-none">
      <AnimatePresence>
        {notifications.map(n => (
          <motion.div
            key={n.id}
            initial={{ x: 60, opacity: 0 }}
            animate={{ x: 0,  opacity: 1 }}
            exit={{    x: 60, opacity: 0 }}
            transition={{ type: 'spring', stiffness: 300, damping: 25 }}
            className={`pointer-events-auto flex items-center gap-2.5 pl-3 pr-2 py-2
                        bg-caiman-bg4 border rounded-lg text-[10px] text-caiman-text
                        max-w-[280px] shadow-panel ${levelBorder[n.level] ?? 'border-caiman-border'}`}
          >
            <div className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
              n.level === 'success' ? 'bg-caiman-bright' :
              n.level === 'warning' ? 'bg-caiman-amber'  :
              n.level === 'error'   ? 'bg-caiman-red'    : 'bg-caiman-blue'
            }`} />
            <span className="flex-1 leading-relaxed">{n.message}</span>
            <button
              onClick={() => clearNotif(n.id)}
              className="text-caiman-dim hover:text-caiman-text flex-shrink-0"
            >
              <X size={10} />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  )
}
