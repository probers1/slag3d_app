import { useCallback, useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { STLLoader } from 'three/examples/jsm/loaders/STLLoader.js';
import { OBJLoader } from 'three/examples/jsm/loaders/OBJLoader.js';
import { ThreeMFLoader } from 'three/examples/jsm/loaders/3MFLoader.js';
import { fetchFileAsBlobUrl } from '../utils/api';
import { ModelFile } from '../types';

export interface MeshStats {
  widthMm: number;
  depthMm: number;
  heightMm: number;
  volumeCm3: number;
  weightGrams: number;
}

interface MeshLoadDeps {
  wireframe: boolean;
  setLoadingMesh: (value: boolean) => void;
  setMeshError: (value: string | null) => void;
  setMeshStats: (stats: MeshStats) => void;
}

/**
 * Owns the Three.js scene, renderer, camera, controls and mesh loading for
 * ModelViewerModal. Extracted from the modal so the component can stay focused
 * on state, selection and UI while the render pipeline lives in one place.
 */
export function useModelViewerScene(mountRef: RefObject<HTMLDivElement | null>) {
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);
  const activeMeshRef = useRef<THREE.Object3D | null>(null);
  const loadGenerationRef = useRef(0);

  const disposeMeshObject = useCallback((object: THREE.Object3D) => {
    sceneRef.current?.remove(object);
    object.traverse((child) => {
      if ((child as THREE.Mesh).isMesh) {
        const mesh = child as THREE.Mesh;
        mesh.geometry?.dispose();
        if (Array.isArray(mesh.material)) {
          mesh.material.forEach((m) => m.dispose());
        } else {
          (mesh.material as THREE.Material | undefined)?.dispose();
        }
      }
    });
  }, []);

  // Initialize Three.js scene once on mount
  useEffect(() => {
    if (!mountRef.current) return;

    const width = mountRef.current.clientWidth || 600;
    const height = mountRef.current.clientHeight || 450;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x121214);
    sceneRef.current = scene;

    const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
    renderer.setSize(width, height);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    rendererRef.current = renderer;

    mountRef.current.innerHTML = '';
    mountRef.current.appendChild(renderer.domElement);

    const camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 5000);
    cameraRef.current = camera;

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controlsRef.current = controls;

    const ambientLight = new THREE.AmbientLight(0xffffff, 0.65);
    scene.add(ambientLight);

    const keyLight = new THREE.DirectionalLight(0xffffff, 1.1);
    keyLight.position.set(1, 2, 1.5).normalize();
    scene.add(keyLight);

    const fillLight = new THREE.DirectionalLight(0x8899bb, 0.4);
    fillLight.position.set(-1, -0.5, -1).normalize();
    scene.add(fillLight);

    const grid = new THREE.GridHelper(200, 20, 0x333344, 0x1c1c24);
    grid.position.y = -0.01;
    scene.add(grid);

    let animId: number;
    const animate = () => {
      animId = requestAnimationFrame(animate);
      controls.update();
      renderer.render(scene, camera);
    };
    animate();

    const handleResize = () => {
      if (!mountRef.current || !cameraRef.current || !rendererRef.current) return;
      const newW = mountRef.current.clientWidth;
      const newH = mountRef.current.clientHeight;
      cameraRef.current.aspect = newW / newH;
      cameraRef.current.updateProjectionMatrix();
      rendererRef.current.setSize(newW, newH);
    };
    window.addEventListener('resize', handleResize);

    return () => {
      loadGenerationRef.current += 1;
      if (activeMeshRef.current) {
        disposeMeshObject(activeMeshRef.current);
        activeMeshRef.current = null;
      }
      cancelAnimationFrame(animId);
      window.removeEventListener('resize', handleResize);
      renderer.dispose();
      if (mountRef.current) {
        mountRef.current.innerHTML = '';
      }
    };
  }, [disposeMeshObject, mountRef]);

  const calculateMeshVolume = useCallback((geometry: THREE.BufferGeometry): number => {
    let position: THREE.BufferAttribute;
    if (geometry.index) {
      const nonIndexed = geometry.toNonIndexed();
      position = nonIndexed.attributes.position as THREE.BufferAttribute;
    } else {
      position = geometry.attributes.position as THREE.BufferAttribute;
    }

    if (!position) return 0;

    let totalVolume = 0;
    const p1 = new THREE.Vector3();
    const p2 = new THREE.Vector3();
    const p3 = new THREE.Vector3();

    for (let i = 0; i < position.count; i += 3) {
      p1.fromBufferAttribute(position, i);
      p2.fromBufferAttribute(position, i + 1);
      p3.fromBufferAttribute(position, i + 2);
      totalVolume += p1.dot(p2.clone().cross(p3)) / 6.0;
    }

    return Math.abs(totalVolume); // in mm³
  }, []);

  const loadActiveMesh = useCallback(
    async (activeFile: ModelFile, deps: MeshLoadDeps): Promise<void> => {
      const scene = sceneRef.current;
      const camera = cameraRef.current;
      const controls = controlsRef.current;
      if (!scene || !camera || !controls) return;

      const generation = ++loadGenerationRef.current;
      const isCurrent = () => generation === loadGenerationRef.current;

      deps.setLoadingMesh(true);
      deps.setMeshError(null);

      if (activeMeshRef.current) {
        disposeMeshObject(activeMeshRef.current);
        activeMeshRef.current = null;
      }

      const material = new THREE.MeshStandardMaterial({
        color: 0x3b82f6,
        roughness: 0.35,
        metalness: 0.25,
        wireframe: deps.wireframe,
      });

      const attachAndPosition = (object: THREE.Object3D, geom?: THREE.BufferGeometry) => {
        if (!isCurrent()) {
          disposeMeshObject(object);
          return;
        }
        scene.add(object);
        activeMeshRef.current = object;

        const box = new THREE.Box3().setFromObject(object);
        const center = box.getCenter(new THREE.Vector3());
        const size = box.getSize(new THREE.Vector3());

        object.position.x += -center.x;
        object.position.y += -center.y + size.y / 2; // Sit flush on grid
        object.position.z += -center.z;

        const maxDim = Math.max(size.x, size.y, size.z) || 50;
        const fov = 45 * (Math.PI / 180);
        const cameraZ = Math.abs(maxDim / 2 / Math.tan(fov / 2)) * 1.5;

        camera.position.set(cameraZ * 0.7, cameraZ * 0.5, cameraZ * 0.7);
        camera.lookAt(0, size.y / 4, 0);
        controls.target.set(0, size.y / 4, 0);
        controls.update();

        // Compute slicing analytics
        let volumeMm3 = 0;
        if (geom) {
          volumeMm3 = calculateMeshVolume(geom);
        } else {
          object.traverse((child) => {
            if ((child as THREE.Mesh).isMesh && (child as THREE.Mesh).geometry) {
              volumeMm3 += calculateMeshVolume((child as THREE.Mesh).geometry);
            }
          });
        }

        const volumeCm3 = volumeMm3 / 1000.0;
        // Standard PLA density = 1.24 g/cm3; assuming effective infill/walls (~45% of solid volume)
        const weightGrams = volumeCm3 * 1.24 * 0.45;

        deps.setMeshStats({
          widthMm: Number(size.x.toFixed(1)),
          depthMm: Number(size.z.toFixed(1)),
          heightMm: Number(size.y.toFixed(1)),
          volumeCm3: Number(volumeCm3.toFixed(2)),
          weightGrams: Number(weightGrams.toFixed(1)),
        });

        deps.setLoadingMesh(false);
      };

      let blobUrl: string | null = null;
      try {
        // Prefer cached blob from the file object to avoid repeated Tauri IPC fetches.
        // Falls back to fetching fresh if blob is missing or stale.
        const ext = activeFile.original_name.split('.').pop()?.toLowerCase();

        // Fetch the blob URL (either cached from before or freshly from Tauri)
        blobUrl = await fetchFileAsBlobUrl(activeFile.file_path);

        if (!isCurrent()) {
          if (blobUrl) URL.revokeObjectURL(blobUrl);
          return;
        }

        if (ext === 'stl') {
          const loader = new STLLoader();
          loader.load(
            blobUrl,
            (geometry) => {
              geometry.computeVertexNormals();
              const mesh = new THREE.Mesh(geometry, material);
              attachAndPosition(mesh, geometry);

              // Cache the raw bytes back onto the file object so subsequent renders reuse it.
              if (!isCurrent()) return;
              activeFile.blob = blobUrl;

              if (blobUrl) URL.revokeObjectURL(blobUrl);
              blobUrl = null;
            },
            undefined,
            (err) => {
              console.error('STL loading error:', err);
              if (blobUrl) URL.revokeObjectURL(blobUrl);
              blobUrl = null;
              if (!isCurrent()) return;
              deps.setMeshError(`Could not parse STL: ${String(err)}`);
              deps.setLoadingMesh(false);
            }
          );
        } else if (ext === 'obj') {
          const loader = new OBJLoader();
          loader.load(
            blobUrl,
            (obj) => {
              obj.traverse((child) => {
                if ((child as THREE.Mesh).isMesh) {
                  (child as THREE.Mesh).material = material;
                }
              });
              attachAndPosition(obj);

              // Cache the raw bytes back onto the file object so subsequent renders reuse it.
              if (!isCurrent()) return;
              activeFile.blob = blobUrl;

              if (blobUrl) URL.revokeObjectURL(blobUrl);
              blobUrl = null;
            },
            undefined,
            (err) => {
              console.error('OBJ loading error:', err);
              if (blobUrl) URL.revokeObjectURL(blobUrl);
              blobUrl = null;
              if (!isCurrent()) return;
              deps.setMeshError(`Could not parse OBJ: ${String(err)}`);
              deps.setLoadingMesh(false);
            }
          );
        } else if (ext === '3mf') {
          const loader = new ThreeMFLoader();
          loader.load(
            blobUrl,
            (group) => {
              group.traverse((child) => {
                if ((child as THREE.Mesh).isMesh) {
                  (child as THREE.Mesh).material = material;
                }
              });
              attachAndPosition(group);

              // Cache the raw bytes back onto the file object so subsequent renders reuse it.
              if (!isCurrent()) return;
              activeFile.blob = blobUrl;

              if (blobUrl) URL.revokeObjectURL(blobUrl);
              blobUrl = null;
            },
            undefined,
            (err) => {
              console.error('3MF loading error:', err);
              if (blobUrl) URL.revokeObjectURL(blobUrl);
              blobUrl = null;
              if (!isCurrent()) return;
              deps.setMeshError(`Could not parse 3MF: ${String(err)}`);
              deps.setLoadingMesh(false);
            }
          );
        } else {
          if (blobUrl) URL.revokeObjectURL(blobUrl);
          blobUrl = null;
          const geom = new THREE.DodecahedronGeometry(25, 0);
          const mesh = new THREE.Mesh(geom, material);
          attachAndPosition(mesh, geom);
        }
      } catch (err) {
        console.error('Failed to load 3D file:', err);
        if (blobUrl) URL.revokeObjectURL(blobUrl);
        blobUrl = null;
        if (!isCurrent()) return;
        deps.setMeshError(`Could not read 3D file: ${err instanceof Error ? err.message : String(err)}`);
        deps.setLoadingMesh(false);
      }
    },
    [disposeMeshObject, fetchFileAsBlobUrl],
  );

  return {
    sceneRef,
    cameraRef,
    rendererRef,
    controlsRef,
    activeMeshRef,
    loadGenerationRef,
    disposeMeshObject,
    calculateMeshVolume,
    loadActiveMesh,
  };
}
