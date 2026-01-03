/**
 * Node Inspection Panel Component
 * Displays detailed information about a selected node including:
 * - Node ID and status
 * - Hyperbolic coordinates
 * - Neighbor list with distances
 * - Routing statistics
 */

import { useMemo } from 'react';
import type { NetworkNode, NetworkTopology, NodeRoutingStats, RoutingMode } from '../types';
import { hyperbolicDistance } from '../utils/hyperbolic';

interface NodeInspectionPanelProps {
  selectedNode: NetworkNode | null;
  topology: NetworkTopology;
  nodeStats: Map<string, NodeRoutingStats>;
  onNodeSelect: (nodeId: string | null) => void;
  onClose: () => void;
}

/** Mode display colors */
const MODE_COLORS: Record<RoutingMode, string> = {
  gravity: '#00ff88',
  pressure: '#ff8800',
  tree: '#ff0088',
};

/**
 * Calculate hyperbolic distance from origin
 */
function distanceFromOrigin(x: number, y: number): number {
  const r = Math.sqrt(x * x + y * y);
  if (r >= 1) return Infinity;
  if (r < 1e-10) return 0;
  return Math.log((1 + r) / (1 - r));
}

export function NodeInspectionPanel({
  selectedNode,
  topology,
  nodeStats,
  onNodeSelect,
  onClose,
}: NodeInspectionPanelProps) {
  // Calculate neighbor distances
  const neighborData = useMemo(() => {
    if (!selectedNode) return [];
    
    return selectedNode.neighbors.map(neighborId => {
      const neighbor = topology.nodes.find(n => n.id === neighborId);
      if (!neighbor) return { id: neighborId, distance: Infinity, isOnline: false };
      
      const dist = hyperbolicDistance(selectedNode.coordinate, neighbor.coordinate);
      return {
        id: neighborId,
        distance: dist,
        isOnline: neighbor.isOnline,
      };
    }).sort((a, b) => a.distance - b.distance);
  }, [selectedNode, topology.nodes]);

  // Get stats for selected node
  const stats = useMemo(() => {
    if (!selectedNode) return null;
    return nodeStats.get(selectedNode.id) || {
      packetsRouted: 0,
      packetsOriginated: 0,
      packetsReceived: 0,
      lastActivity: null,
      modeUsage: { gravity: 0, pressure: 0, tree: 0 },
    };
  }, [selectedNode, nodeStats]);

  // Calculate distance from origin
  const distFromOrigin = useMemo(() => {
    if (!selectedNode) return null;
    return distanceFromOrigin(selectedNode.coordinate.x, selectedNode.coordinate.y);
  }, [selectedNode]);

  // Calculate total mode usage for percentage
  const totalModeUsage = useMemo(() => {
    if (!stats) return 0;
    return stats.modeUsage.gravity + stats.modeUsage.pressure + stats.modeUsage.tree;
  }, [stats]);

  if (!selectedNode) {
    return (
      <div className="node-inspection-panel empty">
        <h2>Node Inspection</h2>
        <p className="hint">Click on a node in the visualization to inspect its details</p>
        <div className="inspection-tips">
          <h3>Tips</h3>
          <ul>
            <li>Click a node to select it</li>
            <li>Click a neighbor to navigate</li>
            <li>View routing statistics per node</li>
            <li>See hyperbolic distances</li>
          </ul>
        </div>
      </div>
    );
  }

  return (
    <div className="node-inspection-panel">
      <div className="panel-header">
        <h2>Node Inspection</h2>
        <button className="close-button" onClick={onClose} title="Close">×</button>
      </div>

      {/* Node Identity */}
      <section className="inspection-section">
        <h3>Identity</h3>
        <div className="info-row">
          <span className="label">Node ID:</span>
          <span className="value monospace">{selectedNode.id}</span>
        </div>
        <div className="info-row">
          <span className="label">Status:</span>
          <span className={`value status ${selectedNode.isOnline ? 'online' : 'offline'}`}>
            {selectedNode.isOnline ? '🟢 Online' : '🔴 Offline'}
          </span>
        </div>
      </section>

      {/* Coordinates */}
      <section className="inspection-section">
        <h3>Hyperbolic Coordinates</h3>
        <div className="coordinates-display">
          <div className="coord-item">
            <span className="coord-label">x:</span>
            <span className="coord-value">{selectedNode.coordinate.x.toFixed(6)}</span>
          </div>
          <div className="coord-item">
            <span className="coord-label">y:</span>
            <span className="coord-value">{selectedNode.coordinate.y.toFixed(6)}</span>
          </div>
        </div>
        <div className="info-row">
          <span className="label">Distance from Origin:</span>
          <span className="value monospace">
            {distFromOrigin !== null ? distFromOrigin.toFixed(4) : 'N/A'}
          </span>
        </div>
        <div className="info-row">
          <span className="label">Euclidean Radius:</span>
          <span className="value monospace">
            {Math.sqrt(
              selectedNode.coordinate.x ** 2 + selectedNode.coordinate.y ** 2
            ).toFixed(4)}
          </span>
        </div>
      </section>

      {/* Neighbors */}
      <section className="inspection-section">
        <h3>Neighbors ({neighborData.length})</h3>
        {neighborData.length > 0 ? (
          <ul className="neighbor-list">
            {neighborData.map(neighbor => (
              <li
                key={neighbor.id}
                className={`neighbor-item ${neighbor.isOnline ? 'online' : 'offline'}`}
                onClick={() => onNodeSelect(neighbor.id)}
              >
                <span className="neighbor-status">
                  {neighbor.isOnline ? '🟢' : '🔴'}
                </span>
                <span className="neighbor-id">{neighbor.id}</span>
                <span className="neighbor-distance">
                  d={neighbor.distance.toFixed(3)}
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="no-neighbors">No neighbors</p>
        )}
      </section>

      {/* Routing Statistics */}
      <section className="inspection-section">
        <h3>Routing Statistics</h3>
        {stats && (
          <>
            <div className="stats-grid">
              <div className="stat-item">
                <span className="stat-value">{stats.packetsRouted}</span>
                <span className="stat-label">Routed</span>
              </div>
              <div className="stat-item">
                <span className="stat-value">{stats.packetsOriginated}</span>
                <span className="stat-label">Originated</span>
              </div>
              <div className="stat-item">
                <span className="stat-value">{stats.packetsReceived}</span>
                <span className="stat-label">Received</span>
              </div>
            </div>

            {totalModeUsage > 0 && (
              <div className="mode-usage">
                <h4>Mode Usage</h4>
                <div className="mode-bar">
                  {(['gravity', 'pressure', 'tree'] as RoutingMode[]).map(mode => {
                    const count = stats.modeUsage[mode];
                    const percentage = (count / totalModeUsage) * 100;
                    if (percentage === 0) return null;
                    return (
                      <div
                        key={mode}
                        className={`mode-segment ${mode}`}
                        style={{ 
                          width: `${percentage}%`,
                          backgroundColor: MODE_COLORS[mode],
                        }}
                        title={`${mode}: ${count} (${percentage.toFixed(1)}%)`}
                      />
                    );
                  })}
                </div>
                <div className="mode-breakdown">
                  {(['gravity', 'pressure', 'tree'] as RoutingMode[]).map(mode => (
                    <div key={mode} className="mode-item">
                      <span 
                        className="mode-dot" 
                        style={{ backgroundColor: MODE_COLORS[mode] }}
                      />
                      <span className="mode-name">{mode}</span>
                      <span className="mode-count">{stats.modeUsage[mode]}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {stats.lastActivity && (
              <div className="info-row">
                <span className="label">Last Activity:</span>
                <span className="value">
                  {new Date(stats.lastActivity).toLocaleTimeString()}
                </span>
              </div>
            )}
          </>
        )}
      </section>
    </div>
  );
}

export default NodeInspectionPanel;
