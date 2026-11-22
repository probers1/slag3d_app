import { describe, it, expect } from 'vitest';
import { errorMessage, mapAssetPaths } from './api';

describe('errorMessage', () => {
  it('passes plain strings through', () => {
    expect(errorMessage('Backup failed')).toBe('Backup failed');
  });

  it('reads the message from a structured AppError-like object', () => {
    expect(errorMessage({ kind: 'archive', message: 'Archive exceeds size limit' })).toBe(
      'Archive exceeds size limit',
    );
  });

  it('reads the message from an Error instance', () => {
    expect(errorMessage(new Error('Failed to load model details'))).toBe('Failed to load model details');
  });

  it('falls back when there is no message', () => {
    expect(errorMessage(undefined)).toBe('Request failed');
    expect(errorMessage({ kind: 'io', message: '' })).toBe('Request failed');
  });
});

describe('mapAssetPaths', () => {
  it('passes plain strings through unchanged outside Tauri', async () => {
    expect(await mapAssetPaths('/uploads/models/abc.stl')).toBe('/uploads/models/abc.stl');
  });

  it('maps thumbnail paths only in Tauri mode (not in the test environment)', async () => {
    // In the node test environment we are not inside Tauri, so thumbnail paths
    // are returned untouched.
    expect(await mapAssetPaths('/uploads/thumbnails/abc.png')).toBe('/uploads/thumbnails/abc.png');
  });

  it('recurses into arrays', async () => {
    expect(await mapAssetPaths(['/uploads/models/a.stl', '/uploads/models/b.stl'])).toEqual([
      '/uploads/models/a.stl',
      '/uploads/models/b.stl',
    ]);
  });

  it('recurses into plain objects', async () => {
    expect(await mapAssetPaths({ model: { title: 'X' } })).toEqual({ model: { title: 'X' } });
  });
});
