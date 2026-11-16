// GeoJSON geometries (RFC 7946) from the bytes a database writes for a geometry, and back: the
// `geometry` codec of the project file's `js.codecs` (src/codecs.ts). PostGIS writes a geometry as
// the hex text of its EWKB, MySQL as a 4-byte little-endian SRID and then its WKB, and DuckDB as
// ISO WKB. Every coordinate is a float64 on both sides, so a geometry of X and Y, or X, Y and Z,
// reads and writes back exactly. One GeoJSON has no form for is refused: M coordinates, which
// `[x, y, m]` would read as Z, curves, triangles, TINs and polyhedral surfaces. The SRID, which
// RFC 7946 dropped with `crs`, rides on the value as a `srid` member, as Prisma's PostGIS
// extension writes it.
//
// This module runs in every generated module's process, so it names no Node type.

/** A GeoJSON geometry, as this module reads and writes one. */
export type Geometry = { type: string; coordinates?: unknown; geometries?: Geometry[] };

/** The seven geometries GeoJSON has, by their WKB type codes, 1 to 7. */
const TYPES = ["Point", "LineString", "Polygon", "MultiPoint", "MultiLineString", "MultiPolygon", "GeometryCollection"] as const;

/** The WKB types GeoJSON has no form for, by their codes, as an error names them. */
const UNSUPPORTED: Readonly<Record<number, string>> = { 8: "CircularString", 9: "CompoundCurve", 10: "CurvePolygon", 11: "MultiCurve", 12: "MultiSurface", 13: "Curve", 14: "Surface", 15: "PolyhedralSurface", 16: "TIN", 17: "Triangle" };

// EWKB's flags in the type code, as PostGIS writes them.
const EWKB_Z = 0x80000000;
const EWKB_M = 0x40000000;
const EWKB_SRID = 0x20000000;

/**
 * A geometry's GeoJSON from its WKB or EWKB `bytes`, and its SRID: the one EWKB carries, else
 * `srid`. `refuse` makes the error for a geometry GeoJSON cannot hold, from why.
 */
export function fromWkb(bytes: Uint8Array, srid: number, refuse: (why: string) => Error): { geometry: Geometry; srid: number } {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let at = 0;
  let found: number | undefined;
  const need = (n: number) => {
    if (at + n > bytes.byteLength) throw refuse(`${bytes.byteLength} bytes, which end inside a geometry`);
  };
  const geometry = (top: boolean): Geometry => {
    need(5);
    const order = view.getUint8(at);
    if (order > 1) throw refuse(`bytes that are no WKB (the byte order byte is ${order})`);
    const le = order === 1;
    const u32 = () => (need(4), (at += 4), view.getUint32(at - 4, le));
    const f64 = () => (need(8), (at += 8), view.getFloat64(at - 8, le));
    at += 1;
    const code = u32();
    let z = (code & EWKB_Z) !== 0;
    let m = (code & EWKB_M) !== 0;
    if (code & EWKB_SRID) {
      const s = u32();
      if (top) found = s;
    }
    let base = code & 0x0fffffff;
    // ISO WKB counts the dimensions in thousands: 1001 is a Point Z, 2001 a Point M, 3001 both.
    if (base >= 1000) {
      const dims = Math.floor(base / 1000);
      base %= 1000;
      z ||= dims === 1 || dims === 3;
      m ||= dims === 2 || dims === 3;
    }
    if (m) throw refuse(`a geometry with M coordinates, which GeoJSON has no place for`);
    const type = TYPES[base - 1];
    if (type === undefined) throw refuse(`a ${UNSUPPORTED[base] ?? `geometry of WKB type ${base}`}, which GeoJSON has no form for`);
    const position = (): number[] => {
      const p = z ? [f64(), f64(), f64()] : [f64(), f64()];
      if (!p.every(Number.isFinite)) throw refuse(`a coordinate that is no finite number, which GeoJSON has no form for`);
      return p;
    };
    const positions = () => Array.from({ length: u32() }, position);
    const members = (kind: string) =>
      Array.from({ length: u32() }, () => {
        const g = geometry(false);
        if (g.type !== kind) throw refuse(`a Multi${kind} holding a ${g.type}`);
        return g.coordinates;
      });
    switch (type) {
      case "Point": {
        const p = z ? [f64(), f64(), f64()] : [f64(), f64()];
        // An empty point is NaN in every coordinate, and an empty array in GeoJSON.
        if (p.every(Number.isNaN)) return { type, coordinates: [] };
        if (!p.every(Number.isFinite)) throw refuse(`a coordinate that is no finite number, which GeoJSON has no form for`);
        return { type, coordinates: p };
      }
      case "LineString":
        return { type, coordinates: positions() };
      case "Polygon":
        return { type, coordinates: Array.from({ length: u32() }, positions) };
      case "MultiPoint": {
        const points = members("Point") as number[][];
        if (points.some((p) => p.length === 0)) throw refuse(`a MultiPoint holding an empty point, which GeoJSON has no form for`);
        return { type, coordinates: points };
      }
      case "MultiLineString":
        return { type, coordinates: members("LineString") };
      case "MultiPolygon":
        return { type, coordinates: members("Polygon") };
      case "GeometryCollection":
        return { type, geometries: Array.from({ length: u32() }, () => geometry(false)) };
    }
  };
  const g = geometry(true);
  if (at !== bytes.byteLength) throw refuse(`${bytes.byteLength - at} bytes past the end of its geometry`);
  return { geometry: g, srid: found ?? srid };
}

/** The bytes of hex text, as PostGIS writes a geometry's EWKB. */
export function fromHex(text: string, refuse: (why: string) => Error): Uint8Array {
  if (text.length % 2 !== 0 || !/^[0-9A-Fa-f]*$/.test(text)) throw refuse(`text that is no hex EWKB`);
  const out = new Uint8Array(text.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(text.slice(i * 2, i * 2 + 2), 16);
  return out;
}

/** Bytes as hex text, which PostGIS reads as a geometry's EWKB. */
export function toHex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += (b < 16 ? "0" : "") + b.toString(16);
  return s.toUpperCase();
}

/**
 * A GeoJSON geometry's little-endian WKB: EWKB carrying `srid` where `ewkb` and the SRID is not 0,
 * as PostGIS reads it, and ISO WKB otherwise, its Z in thousands, as MySQL and DuckDB read it.
 * `refuse` makes the error for a value that is no GeoJSON geometry this writes.
 */
export function toWkb(g: unknown, srid: number, ewkb: boolean, refuse: (why: string) => Error): Uint8Array {
  const out: number[] = [];
  const scratch = new DataView(new ArrayBuffer(8));
  const u32 = (n: number) => {
    scratch.setUint32(0, n, true);
    for (let i = 0; i < 4; i++) out.push(scratch.getUint8(i));
  };
  const f64 = (n: number) => {
    scratch.setFloat64(0, n, true);
    for (let i = 0; i < 8; i++) out.push(scratch.getUint8(i));
  };
  // Every position of a geometry has the same dimensions, two or three.
  let dims: 2 | 3 | undefined;
  const position = (p: unknown) => {
    if (!Array.isArray(p) || (p.length !== 2 && p.length !== 3) || !p.every((n) => typeof n === "number" && Number.isFinite(n))) throw refuse(`a position that is no array of two or three finite numbers: ${show(p)}`);
    dims ??= p.length as 2 | 3;
    if (p.length !== dims) throw refuse(`positions of both two and three coordinates`);
  };
  const list = (v: unknown, depth: number, what: string) => {
    if (!Array.isArray(v)) throw refuse(`${what} whose coordinates are no array: ${show(v)}`);
    for (const e of v) {
      if (depth === 0) position(e);
      else list(e, depth - 1, what);
    }
  };
  // The dimensions are read off every position first, so that a header names them.
  const scan = (v: unknown) => {
    if (v === null || typeof v !== "object" || Array.isArray(v)) throw refuse(`a value that is no GeoJSON geometry: ${show(v)}`);
    const o = v as Geometry;
    const depth = { Point: -1, LineString: 0, Polygon: 1, MultiPoint: 0, MultiLineString: 1, MultiPolygon: 2 }[o.type];
    if (o.type === "GeometryCollection") {
      if (!Array.isArray(o.geometries)) throw refuse(`a GeometryCollection whose geometries are no array`);
      o.geometries.forEach(scan);
    } else if (depth === undefined) throw refuse(`a value whose type is ${show(o.type)}, which is none of GeoJSON's geometries`);
    else if (depth === -1) {
      if (!Array.isArray(o.coordinates)) throw refuse(`a Point whose coordinates are no array: ${show(o.coordinates)}`);
      if (o.coordinates.length) position(o.coordinates);
    } else list(o.coordinates, depth, `a ${o.type}`);
  };
  scan(g);
  const z = dims === 3;
  const write = (v: Geometry, top: boolean) => {
    const base = TYPES.indexOf(v.type as (typeof TYPES)[number]) + 1;
    out.push(1);
    if (ewkb) {
      u32((base | (z ? EWKB_Z : 0) | (top && srid !== 0 ? EWKB_SRID : 0)) >>> 0);
      if (top && srid !== 0) u32(srid);
    } else u32(base + (z ? 1000 : 0));
    const position = (p: number[]) => p.forEach(f64);
    const positions = (ps: number[][]) => (u32(ps.length), ps.forEach(position));
    const c = v.coordinates as never;
    switch (v.type) {
      case "Point": {
        const p = c as number[];
        if (p.length) position(p);
        else for (let i = 0; i < (z ? 3 : 2); i++) f64(NaN);
        return;
      }
      case "LineString":
        return positions(c);
      case "Polygon":
        return u32((c as number[][][]).length), (c as number[][][]).forEach(positions);
      case "MultiPoint":
        return u32((c as number[][]).length), (c as number[][]).forEach((p) => write({ type: "Point", coordinates: p }, false));
      case "MultiLineString":
        return u32((c as number[][][]).length), (c as number[][][]).forEach((l) => write({ type: "LineString", coordinates: l }, false));
      case "MultiPolygon":
        return u32((c as number[][][][]).length), (c as number[][][][]).forEach((p) => write({ type: "Polygon", coordinates: p }, false));
      default:
        return u32(v.geometries!.length), v.geometries!.forEach((g) => write(g, false));
    }
  };
  write(g as Geometry, true);
  return Uint8Array.from(out);
}

/** A value as an error shows it, cut short. */
function show(v: unknown): string {
  let s: string;
  try {
    s = JSON.stringify(v) ?? String(v);
  } catch {
    s = String(v);
  }
  return s.length > 60 ? `${s.slice(0, 57)}...` : s;
}
