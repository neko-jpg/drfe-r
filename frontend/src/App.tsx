/**
 * DRFE-R Visualization Dashboard
 * Main application component
 */

import { useState } from 'react';
import { PoincareDisk } from './components';
import type { NetworkTopology } from './types';
import './App.css';

// Generate a more comprehensive sample topology for demonstration
function generateSampleTopology(): NetworkTopology {
  const nodes = [
    // Central node (root)
    { id: 'node-0', coordinate: { x: 0, y: 0 }, neighbors: ['node-1', 'node-2', 'node-3', 'node-4'], isOnline: true },
    // First ring (closer to center)
    { id: 'node-1', coordinate: { x: 0.25, y: 0.3 }, neighbors: ['node-0', 'node-5', 'node-6'], isOnline: true },
    { id: 'node-2', coordinate: { x: -0.3, y: 0.2 }, neighbors: ['node-0', 'node-7', 'node-8'], isOnline: true },
    { id: 'node-3', coordinate: { x: 0.15, y: -0.35 }, neighbors: ['node-0', 'node-9', 'node-10'], isOnline: true },
    { id: 'node-4', coordinate: { x: -0.2, y: -0.25 }, neighbors: ['node-0', 'node-11'], isOnline: true },
    // Second ring (further from center)
    { id: 'node-5', coordinate: { x: 0.5, y: 0.4 }, neighbors: ['node-1', 'node-12'], isOnline: true },
    { id: 'node-6', coordinate: { x: 0.4, y: 0.55 }, neighbors: ['node-1', 'node-13'], isOnline: true },
    { id: 'node-7', coordinate: { x: -0.55, y: 0.35 }, neighbors: ['node-2', 'node-14'], isOnline: true },
    { id: 'node-8', coordinate: { x: -0.45, y: 0.5 }, neighbors: ['node-2'], isOnline: false },
    { id: 'node-9', coordinate: { x: 0.35, y: -0.55 }, neighbors: ['node-3', 'node-15'], isOnline: true },
    { id: 'node-10', coordinate: { x: 0.5, y: -0.4 }, neighbors: ['node-3'], isOnline: true },
    { id: 'node-11', coordinate: { x: -0.4, y: -0.5 }, neighbors: ['node-4', 'node-16'], isOnline: true },
    // Third ring (near boundary)
    { id: 'node-12', coordinate: { x: 0.7, y: 0.45 }, neighbors: ['node-5'], isOnline: true },
    { id: 'node-13', coordinate: { x: 0.55, y: 0.7 }, neighbors: ['node-6'], isOnline: true },
    { id: 'node-14', coordinate: { x: -0.75, y: 0.4 }, neighbors: ['node-7'], isOnline: true },
    { id: 'node-15', coordinate: { x: 0.45, y: -0.72 }, neighbors: ['node-9'], isOnline: false },
    { id: 'node-16', coordinate: { x: -0.6, y: -0.65 }, neighbors: ['node-11'], isOnline: true },
  ];

  const edges = [
    // From center
    { source: 'node-0', target: 'node-1', curvature: 0.1 },
    { source: 'node-0', target: 'node-2', curvature: -0.15 },
    { source: 'node-0', target: 'node-3', curvature: 0.05 },
    { source: 'node-0', target: 'node-4', curvature: -0.1 },
    // First to second ring
    { source: 'node-1', target: 'node-5', curvature: 0.2 },
    { source: 'node-1', target: 'node-6', curvature: 0.15 },
    { source: 'node-2', target: 'node-7', curvature: -0.2 },
    { source: 'node-2', target: 'node-8', curvature: -0.1 },
    { source: 'node-3', target: 'node-9', curvature: 0.25 },
    { source: 'node-3', target: 'node-10', curvature: 0.1 },
    { source: 'node-4', target: 'node-11', curvature: -0.15 },
    // Second to third ring
    { source: 'node-5', target: 'node-12', curvature: 0.3 },
    { source: 'node-6', target: 'node-13', curvature: 0.25 },
    { source: 'node-7', target: 'node-14', curvature: -0.3 },
    { source: 'node-9', target: 'node-15', curvature: 0.35 },
    { source: 'node-11', target: 'node-16', curvature: -0.25 },
  ];

  return { nodes, edges };
}

const sampleTopology = generateSampleTopology();

function App() {
  const [selectedNode, setSelectedNode] = useState<string | null>(null);
  const [showCurvature, setShowCurvature] = useState(false);
  const [showLabels, setShowLabels] = useState(true);
  const [showEdges, setShowEdges] = useState(true);

  const selectedNodeData = selectedNode
    ? sampleTopology.nodes.find(n => n.id === selectedNode)
    : null;

  return (
    <div className="app">
      <header className="header">
        <h1>DRFE-R Visualization Dashboard</h1>
        <p>Poincaré Disk Model - Hyperbolic Routing Visualization</p>
      </header>

      <main className="main">
        <div className="visualization-container">
          <PoincareDisk
            topology={sampleTopology}
            selectedNode={selectedNode ?? undefined}
            onNodeSelect={setSelectedNode}
            options={{
              width: 600,
              height: 600,
              showCurvature,
              showLabels,
              showEdges,
            }}
          />
        </div>

        <aside className="sidebar">
          <section className="controls">
            <h2>Display Options</h2>
            <label>
              <input
                type="checkbox"
                checked={showEdges}
                onChange={e => setShowEdges(e.target.checked)}
              />
              Show Edges
            </label>
            <label>
              <input
                type="checkbox"
                checked={showLabels}
                onChange={e => setShowLabels(e.target.checked)}
              />
              Show Labels
            </label>
            <label>
              <input
                type="checkbox"
                checked={showCurvature}
                onChange={e => setShowCurvature(e.target.checked)}
              />
              Show Curvature Heatmap
            </label>
            <div className="controls-hint">
              <small>
                🖱️ Scroll to zoom • Drag to pan • Double-click to reset
              </small>
            </div>
          </section>

          <section className="node-info">
            <h2>Node Information</h2>
            {selectedNodeData ? (
              <div className="node-details">
                <p><strong>ID:</strong> {selectedNodeData.id}</p>
                <p><strong>Status:</strong> {selectedNodeData.isOnline ? '🟢 Online' : '🔴 Offline'}</p>
                <p><strong>Coordinates:</strong></p>
                <p className="coords">
                  x: {selectedNodeData.coordinate.x.toFixed(4)}<br />
                  y: {selectedNodeData.coordinate.y.toFixed(4)}
                </p>
                <p><strong>Neighbors:</strong> {selectedNodeData.neighbors.length}</p>
                <ul className="neighbors">
                  {selectedNodeData.neighbors.map(n => (
                    <li key={n} onClick={() => setSelectedNode(n)}>{n}</li>
                  ))}
                </ul>
              </div>
            ) : (
              <p className="hint">Click on a node to view details</p>
            )}
          </section>

          <section className="stats">
            <h2>Network Statistics</h2>
            <p><strong>Total Nodes:</strong> {sampleTopology.nodes.length}</p>
            <p><strong>Total Edges:</strong> {sampleTopology.edges.length}</p>
            <p><strong>Online Nodes:</strong> {sampleTopology.nodes.filter(n => n.isOnline).length}</p>
            <p><strong>Offline Nodes:</strong> {sampleTopology.nodes.filter(n => !n.isOnline).length}</p>
          </section>

          <section className="legend">
            <h2>Legend</h2>
            <div className="legend-item">
              <span className="legend-dot online"></span>
              <span>Online Node</span>
            </div>
            <div className="legend-item">
              <span className="legend-dot offline"></span>
              <span>Offline Node</span>
            </div>
            <div className="legend-item">
              <span className="legend-dot selected"></span>
              <span>Selected Node</span>
            </div>
            {showCurvature && (
              <>
                <div className="legend-item">
                  <span className="legend-line negative"></span>
                  <span>Negative Curvature</span>
                </div>
                <div className="legend-item">
                  <span className="legend-line positive"></span>
                  <span>Positive Curvature</span>
                </div>
              </>
            )}
          </section>
        </aside>
      </main>

      <footer className="footer">
        <p>DRFE-R - Distributed Ricci Flow Embedding with Rendezvous Mechanism</p>
      </footer>
    </div>
  );
}

export default App;
