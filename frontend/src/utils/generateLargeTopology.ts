/**
 * Utility to generate large network topologies for testing
 * Used to verify visualization performance with 1000+ nodes
 */

import type { NetworkTopology, NetworkNode, NetworkEdge } from '../types';

/**
 * Generate a Barabási-Albert scale-free network topology
 * This creates a realistic network structure for testing
 */
export function generateBATopology(
  numNodes: number,
  initialNodes: number = 3,
  edgesPerNewNode: number = 2
): NetworkTopology {
  const nodes: NetworkNode[] = [];
  const edges: NetworkEdge[] = [];
  const adjacency: Map<string, Set<string>> = new Map();

  // Helper to add edge
  const addEdge = (source: string, target: string) => {
    if (source === target) return;
    if (!adjacency.has(source)) adjacency.set(source, new Set());
    if (!adjacency.has(target)) adjacency.set(target, new Set());
    if (adjacency.get(source)!.has(target)) return;
    
    adjacency.get(source)!.add(target);
    adjacency.get(target)!.add(source);
    
    // Random curvature between -0.5 and 0.5
    const curvature = (Math.random() - 0.5);
    edges.push({ source, target, curvature });
  };

  // Create initial fully connected clique
  for (let i = 0; i < initialNodes; i++) {
    const id = `node-${i}`;
    // Place initial nodes near center
    const angle = (2 * Math.PI * i) / initialNodes;
    const radius = 0.1 + Math.random() * 0.1;
    nodes.push({
      id,
      coordinate: {
        x: radius * Math.cos(angle),
        y: radius * Math.sin(angle),
      },
      neighbors: [],
      isOnline: true,
    });
    adjacency.set(id, new Set());
  }

  // Connect initial nodes
  for (let i = 0; i < initialNodes; i++) {
    for (let j = i + 1; j < initialNodes; j++) {
      addEdge(`node-${i}`, `node-${j}`);
    }
  }

  // Add remaining nodes using preferential attachment
  for (let i = initialNodes; i < numNodes; i++) {
    const id = `node-${i}`;
    
    // Calculate position in Poincaré disk
    // Use a spiral pattern to distribute nodes
    const t = i / numNodes;
    const spiralAngle = 6 * Math.PI * t + Math.random() * 0.5;
    const spiralRadius = 0.1 + 0.85 * Math.sqrt(t) * (0.9 + Math.random() * 0.1);
    
    // Ensure point is inside disk
    const clampedRadius = Math.min(spiralRadius, 0.95);
    
    nodes.push({
      id,
      coordinate: {
        x: clampedRadius * Math.cos(spiralAngle),
        y: clampedRadius * Math.sin(spiralAngle),
      },
      neighbors: [],
      isOnline: Math.random() > 0.05, // 95% online
    });
    adjacency.set(id, new Set());

    // Calculate degree sum for preferential attachment
    let degreeSum = 0;
    for (let j = 0; j < i; j++) {
      degreeSum += adjacency.get(`node-${j}`)!.size;
    }

    // Connect to existing nodes with preferential attachment
    const targets = new Set<string>();
    let attempts = 0;
    while (targets.size < edgesPerNewNode && attempts < 100) {
      attempts++;
      let r = Math.random() * degreeSum;
      for (let j = 0; j < i; j++) {
        const targetId = `node-${j}`;
        r -= adjacency.get(targetId)!.size;
        if (r <= 0 && !targets.has(targetId)) {
          targets.add(targetId);
          break;
        }
      }
    }

    // Add edges
    targets.forEach(target => addEdge(id, target));
  }

  // Update neighbor lists
  nodes.forEach(node => {
    node.neighbors = Array.from(adjacency.get(node.id) || []);
  });

  return { nodes, edges };
}

/**
 * Generate a grid topology in hyperbolic space
 */
export function generateGridTopology(
  rows: number,
  cols: number
): NetworkTopology {
  const nodes: NetworkNode[] = [];
  const edges: NetworkEdge[] = [];

  // Create nodes
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const id = `node-${r}-${c}`;
      
      // Map grid position to Poincaré disk
      const x = (c / (cols - 1) - 0.5) * 1.6;
      const y = (r / (rows - 1) - 0.5) * 1.6;
      
      // Apply hyperbolic transformation to fit in disk
      const norm = Math.sqrt(x * x + y * y);
      const scale = norm > 0 ? Math.tanh(norm) / norm : 1;
      
      nodes.push({
        id,
        coordinate: {
          x: x * scale * 0.9,
          y: y * scale * 0.9,
        },
        neighbors: [],
        isOnline: true,
      });
    }
  }

  // Create edges (4-connected grid)
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const id = `node-${r}-${c}`;
      const neighbors: string[] = [];

      if (r > 0) {
        const neighbor = `node-${r - 1}-${c}`;
        neighbors.push(neighbor);
        edges.push({ source: id, target: neighbor, curvature: 0 });
      }
      if (c > 0) {
        const neighbor = `node-${r}-${c - 1}`;
        neighbors.push(neighbor);
        edges.push({ source: id, target: neighbor, curvature: 0 });
      }
      if (r < rows - 1) neighbors.push(`node-${r + 1}-${c}`);
      if (c < cols - 1) neighbors.push(`node-${r}-${c + 1}`);

      const node = nodes.find(n => n.id === id);
      if (node) node.neighbors = neighbors;
    }
  }

  return { nodes, edges };
}

/**
 * Generate a random topology with specified number of nodes
 */
export function generateRandomTopology(
  numNodes: number,
  avgDegree: number = 4
): NetworkTopology {
  const nodes: NetworkNode[] = [];
  const edges: NetworkEdge[] = [];
  const adjacency: Map<string, Set<string>> = new Map();

  // Create nodes with random positions in Poincaré disk
  for (let i = 0; i < numNodes; i++) {
    const id = `node-${i}`;
    
    // Random position in disk using rejection sampling
    let x, y;
    do {
      x = (Math.random() - 0.5) * 1.9;
      y = (Math.random() - 0.5) * 1.9;
    } while (x * x + y * y >= 0.95 * 0.95);

    nodes.push({
      id,
      coordinate: { x, y },
      neighbors: [],
      isOnline: Math.random() > 0.02,
    });
    adjacency.set(id, new Set());
  }

  // Create random edges
  const targetEdges = Math.floor((numNodes * avgDegree) / 2);
  let edgeCount = 0;
  let attempts = 0;

  while (edgeCount < targetEdges && attempts < targetEdges * 10) {
    attempts++;
    const i = Math.floor(Math.random() * numNodes);
    const j = Math.floor(Math.random() * numNodes);
    
    if (i === j) continue;
    
    const source = `node-${i}`;
    const target = `node-${j}`;
    
    if (adjacency.get(source)!.has(target)) continue;
    
    adjacency.get(source)!.add(target);
    adjacency.get(target)!.add(source);
    
    edges.push({
      source,
      target,
      curvature: (Math.random() - 0.5) * 0.8,
    });
    edgeCount++;
  }

  // Update neighbor lists
  nodes.forEach(node => {
    node.neighbors = Array.from(adjacency.get(node.id) || []);
  });

  return { nodes, edges };
}

/**
 * Performance test: measure rendering time for large topologies
 */
export function measureTopologyStats(topology: NetworkTopology): {
  nodeCount: number;
  edgeCount: number;
  avgDegree: number;
  onlineRatio: number;
  boundaryNodes: number;
} {
  const nodeCount = topology.nodes.length;
  const edgeCount = topology.edges.length;
  const avgDegree = (2 * edgeCount) / nodeCount;
  const onlineCount = topology.nodes.filter(n => n.isOnline).length;
  const onlineRatio = onlineCount / nodeCount;
  
  // Count nodes near boundary (|z| > 0.8)
  const boundaryNodes = topology.nodes.filter(n => {
    const norm = Math.sqrt(n.coordinate.x ** 2 + n.coordinate.y ** 2);
    return norm > 0.8;
  }).length;

  return {
    nodeCount,
    edgeCount,
    avgDegree,
    onlineRatio,
    boundaryNodes,
  };
}
