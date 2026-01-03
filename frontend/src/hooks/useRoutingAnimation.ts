/**
 * Hook for managing packet routing animations
 * 
 * Handles:
 * - Adding new packet animations from routing events
 * - Updating animation progress over time
 * - Removing completed animations
 * - Tracking active routing mode
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import type { PacketAnimation, RoutingEvent, RoutingMode, RoutingStats } from '../types';

interface UseRoutingAnimationOptions {
  /** Animation duration in milliseconds */
  animationDuration?: number;
  /** Maximum concurrent animations */
  maxAnimations?: number;
  /** Frame rate for animation updates (fps) */
  frameRate?: number;
}

interface UseRoutingAnimationReturn {
  /** Current active animations */
  animations: PacketAnimation[];
  /** Current active routing mode */
  activeMode: RoutingMode | null;
  /** Routing statistics */
  stats: RoutingStats;
  /** Add a routing event to animate */
  addRoutingEvent: (event: RoutingEvent) => void;
  /** Clear all animations */
  clearAnimations: () => void;
  /** Reset statistics */
  resetStats: () => void;
  /** Whether animations are currently playing */
  isAnimating: boolean;
}

const DEFAULT_ANIMATION_DURATION = 500;
const DEFAULT_MAX_ANIMATIONS = 20;
const DEFAULT_FRAME_RATE = 60;

/**
 * Hook for managing packet routing animations
 */
export function useRoutingAnimation(
  options: UseRoutingAnimationOptions = {}
): UseRoutingAnimationReturn {
  const {
    animationDuration = DEFAULT_ANIMATION_DURATION,
    maxAnimations = DEFAULT_MAX_ANIMATIONS,
    frameRate = DEFAULT_FRAME_RATE,
  } = options;

  const [animations, setAnimations] = useState<PacketAnimation[]>([]);
  const [activeMode, setActiveMode] = useState<RoutingMode | null>(null);
  const [stats, setStats] = useState<RoutingStats>({
    totalPackets: 0,
    deliveredPackets: 0,
    failedPackets: 0,
    averageHops: 0,
    modeBreakdown: {
      gravity: 0,
      pressure: 0,
      tree: 0,
    },
  });

  const animationFrameRef = useRef<number | null>(null);
  const lastUpdateRef = useRef<number>(0);
  const hopCountsRef = useRef<number[]>([]);

  // Update animation progress
  const updateAnimations = useCallback(() => {
    const now = performance.now();
    
    setAnimations(prevAnimations => {
      const updatedAnimations = prevAnimations
        .map(anim => {
          const elapsed = now - anim.startTime;
          const progress = Math.min(1, elapsed / anim.duration);
          return { ...anim, progress };
        })
        .filter(anim => anim.progress < 1);

      // Update active mode based on most recent animation
      if (updatedAnimations.length > 0) {
        const mostRecent = updatedAnimations.reduce((latest, anim) =>
          anim.startTime > latest.startTime ? anim : latest
        );
        setActiveMode(mostRecent.mode);
      } else {
        setActiveMode(null);
      }

      return updatedAnimations;
    });
  }, []);

  // Animation loop
  useEffect(() => {
    if (animations.length === 0) {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
      return;
    }

    const frameInterval = 1000 / frameRate;

    const animate = (timestamp: number) => {
      if (timestamp - lastUpdateRef.current >= frameInterval) {
        updateAnimations();
        lastUpdateRef.current = timestamp;
      }
      animationFrameRef.current = requestAnimationFrame(animate);
    };

    animationFrameRef.current = requestAnimationFrame(animate);

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [animations.length, frameRate, updateAnimations]);

  // Add a routing event to animate
  const addRoutingEvent = useCallback((event: RoutingEvent) => {
    const now = performance.now();

    // Handle different event types
    switch (event.eventType) {
      case 'packet_hop':
      case 'packet_sent': {
        // Create new animation
        const newAnimation: PacketAnimation = {
          id: `${event.packetId}-${event.timestamp}`,
          fromNode: event.fromNode,
          toNode: event.toNode,
          mode: event.mode,
          progress: 0,
          startTime: now,
          duration: animationDuration,
        };

        setAnimations(prev => {
          // Limit concurrent animations
          const filtered = prev.length >= maxAnimations
            ? prev.slice(1)
            : prev;
          return [...filtered, newAnimation];
        });

        // Update mode breakdown
        setStats(prev => ({
          ...prev,
          modeBreakdown: {
            ...prev.modeBreakdown,
            [event.mode]: prev.modeBreakdown[event.mode] + 1,
          },
        }));

        setActiveMode(event.mode);
        break;
      }

      case 'packet_delivered': {
        // Update statistics
        if (event.hops !== undefined) {
          hopCountsRef.current.push(event.hops);
          const avgHops = hopCountsRef.current.reduce((a, b) => a + b, 0) / hopCountsRef.current.length;
          
          setStats(prev => ({
            ...prev,
            totalPackets: prev.totalPackets + 1,
            deliveredPackets: prev.deliveredPackets + 1,
            averageHops: avgHops,
          }));
        } else {
          setStats(prev => ({
            ...prev,
            totalPackets: prev.totalPackets + 1,
            deliveredPackets: prev.deliveredPackets + 1,
          }));
        }
        break;
      }

      case 'packet_failed': {
        setStats(prev => ({
          ...prev,
          totalPackets: prev.totalPackets + 1,
          failedPackets: prev.failedPackets + 1,
        }));
        break;
      }

      case 'mode_change': {
        setActiveMode(event.mode);
        break;
      }
    }
  }, [animationDuration, maxAnimations]);

  // Clear all animations
  const clearAnimations = useCallback(() => {
    setAnimations([]);
    setActiveMode(null);
  }, []);

  // Reset statistics
  const resetStats = useCallback(() => {
    hopCountsRef.current = [];
    setStats({
      totalPackets: 0,
      deliveredPackets: 0,
      failedPackets: 0,
      averageHops: 0,
      modeBreakdown: {
        gravity: 0,
        pressure: 0,
        tree: 0,
      },
    });
  }, []);

  return {
    animations,
    activeMode,
    stats,
    addRoutingEvent,
    clearAnimations,
    resetStats,
    isAnimating: animations.length > 0,
  };
}

export default useRoutingAnimation;
