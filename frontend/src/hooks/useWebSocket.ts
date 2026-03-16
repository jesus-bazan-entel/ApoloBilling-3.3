import { useEffect, useRef, useState, useCallback } from 'react'
import type { WSMessage, ActiveCall, DashboardStats } from '../types'

interface UseWebSocketOptions {
  onCallStart?: (call: ActiveCall) => void
  onCallUpdate?: (call: ActiveCall) => void
  onCallEnd?: (call: ActiveCall) => void
  onStatsUpdate?: (stats: DashboardStats) => void
  reconnectInterval?: number
}

export function useWebSocket(options: UseWebSocketOptions = {}) {
  const {
    onCallStart,
    onCallUpdate,
    onCallEnd,
    onStatsUpdate,
    reconnectInterval = 3000,
  } = options

  const [isConnected, setIsConnected] = useState(false)
  const [activeCalls, setActiveCalls] = useState<ActiveCall[]>([])
  const [stats, setStats] = useState<DashboardStats | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimeoutRef = useRef<number | null>(null)
  const connectRef = useRef<(() => void) | null>(null)

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}/ws`

    try {
      wsRef.current = new WebSocket(wsUrl)

      wsRef.current.onopen = () => {
        console.log('WebSocket connected')
        setIsConnected(true)
      }

      wsRef.current.onmessage = (event) => {
        try {
          const message: WSMessage = JSON.parse(event.data)

          switch (message.type) {
            case 'active_calls': {
              // Full list of active calls from server
              const allCalls = message.data as ActiveCall[]
              setActiveCalls(allCalls)
              break
            }

            case 'call_start': {
              const newCall = message.data as ActiveCall
              setActiveCalls((prev) => [...prev, newCall])
              onCallStart?.(newCall)
              break
            }

            case 'call_update': {
              const updatedCall = message.data as ActiveCall
              setActiveCalls((prev) =>
                prev.map((c) => (c.call_uuid === updatedCall.call_uuid ? updatedCall : c))
              )
              onCallUpdate?.(updatedCall)
              break
            }

            case 'call_end': {
              const endedCall = message.data as ActiveCall
              setActiveCalls((prev) => prev.filter((c) => c.call_uuid !== endedCall.call_uuid))
              onCallEnd?.(endedCall)
              break
            }

            case 'stats_update': {
              const newStats = message.data as DashboardStats
              setStats(newStats)
              onStatsUpdate?.(newStats)
              break
            }

            case 'pong':
              // Keepalive response, ignore
              break
          }
        } catch (err) {
          console.error('Failed to parse WebSocket message:', err)
        }
      }

      wsRef.current.onclose = () => {
        console.log('WebSocket disconnected')
        setIsConnected(false)
        // Attempt to reconnect using ref to avoid circular dependency
        reconnectTimeoutRef.current = window.setTimeout(() => {
          connectRef.current?.()
        }, reconnectInterval)
      }

      wsRef.current.onerror = (error) => {
        console.error('WebSocket error:', error)
      }
    } catch (err) {
      console.error('Failed to create WebSocket:', err)
      reconnectTimeoutRef.current = window.setTimeout(() => {
        connectRef.current?.()
      }, reconnectInterval)
    }
  }, [onCallStart, onCallUpdate, onCallEnd, onStatsUpdate, reconnectInterval])

  // Update connectRef when connect changes
  useEffect(() => {
    connectRef.current = connect
  }, [connect])

  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
    }
    if (wsRef.current) {
      wsRef.current.close()
      wsRef.current = null
    }
  }, [])

  const send = useCallback((message: object) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message))
    }
  }, [])

  useEffect(() => {
    connect()
    return () => disconnect()
  }, [connect, disconnect])

  return {
    isConnected,
    activeCalls,
    stats,
    send,
    reconnect: connect,
  }
}
