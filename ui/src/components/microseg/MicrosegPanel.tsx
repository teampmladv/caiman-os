import React from 'react'
import type { AuditEvent } from '../../types'

export function MicrosegPanel({ events, deniesTotal }: {
  events: AuditEvent[]; deniesTotal: number
}) {
  return (
    <div className="panel">
      <div className="panel-hd">
        <span className="panel-title">Micro-seg · XDP</span>
        <span className="text-[9px] text-caiman-red">{deniesTotal} denies/60s</span>
      </div>
      <div className="panel-body">
        {events.slice(0, 5).map((e, i) => (
          <div key={i} className="flex items-center gap-2 py-1 border-b border-caiman-bg last:border-b-0 text-[9px]">
            <span className={e.verdict === 'DENY' ? 'text-caiman-red min-w-[36px]' : 'text-caiman-bright min-w-[36px]'}>
              {e.verdict}
            </span>
            <span className="text-caiman-text flex-1 truncate">{e.srcIp} → {e.dstIp}</span>
            <span className="text-caiman-dim">:{e.dstPort}</span>
          </div>
        ))}
        {events.length === 0 && (
          <div className="text-[9px] text-caiman-dim py-2 text-center">No recent events</div>
        )}
      </div>
    </div>
  )
}
