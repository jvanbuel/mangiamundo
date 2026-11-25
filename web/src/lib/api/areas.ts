import type { Area, AreaProgress, Catalog, Celebration, Discover, Health, Located } from '../types';
import { area, raw, read, tz } from './client';

export const health = () => raw<Health>('GET', '/api/health', null);

export const catalog = () => raw<Catalog>('GET', '/api/countries', null);

export const discover = (areaId: string) => read<Discover>(`/api/discover?${area(areaId)}&${tz()}`);

/** Cities and postal codes matching what someone typed. */
export const searchAreas = (query: string) => raw<Area[]>('GET', `/api/areas?q=${encodeURIComponent(query)}`, null);

/** Suggestions while someone types, nearest first when `near` is known. */
export const suggestAreas = (query: string, near: { lat: number; lng: number } | null) =>
	raw<Area[]>(
		'GET',
		`/api/areas/suggest?q=${encodeURIComponent(query)}${near ? `&lat=${near.lat}&lng=${near.lng}` : ''}`,
		null
	);

/** How far loading an area has come while discover waits on it; null when it isn't loading. */
export const areaProgress = (areaId: string) =>
	raw<AreaProgress | null>('GET', `/api/areas/${encodeURIComponent(areaId)}/progress`, null);

/** The city and postal code a point lies in. */
export const locateArea = (lat: number, lng: number) =>
	raw<Located>('GET', `/api/areas/locate?lat=${lat}&lng=${lng}`, null);

export const celebrations = (areaId: string, days = 60) =>
	raw<Celebration[]>('GET', `/api/celebrations?${area(areaId)}&days=${days}&${tz()}`, null);
