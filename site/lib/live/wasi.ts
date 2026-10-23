// The smallest WASI preview1 a Reylo engine needs in a browser: no files, no preopens, an empty
// environment, stdout/stderr to the console, random bytes from crypto and the clock. The Postgres
// engine imports 11 functions, SQLite's 25 (its embedded SQLite links the file calls, which an
// in-memory database never makes), MySQL's 5. Anything the engine asks of a file is EBADF.
const ESUCCESS = 0, EBADF = 8, ENOSYS = 52;

export class ProcExit extends Error {
  constructor(public code: number) {
    super(`the engine exited with ${code}`);
  }
}

export function wasiImports(memory: () => WebAssembly.Memory, log: (fd: number, text: string) => void = (fd, t) => (fd === 2 ? console.error : console.log)(t)) {
  const view = () => new DataView(memory().buffer);
  const bytes = () => new Uint8Array(memory().buffer);
  const decoder = new TextDecoder();
  const pending: Record<number, string> = { 1: "", 2: "" };
  const start = performance.now();
  const unsupported = () => EBADF;
  return {
    random_get(ptr: number, len: number) {
      for (let at = 0; at < len; at += 65536) crypto.getRandomValues(new Uint8Array(memory().buffer, ptr + at, Math.min(65536, len - at)));
      return ESUCCESS;
    },
    environ_sizes_get(countPtr: number, sizePtr: number) {
      const v = view();
      v.setUint32(countPtr, 0, true);
      v.setUint32(sizePtr, 0, true);
      return ESUCCESS;
    },
    environ_get() {
      return ESUCCESS;
    },
    clock_time_get(id: number, _precision: bigint, out: number) {
      // 0 is the realtime clock; every other is monotonic.
      const ns = id === 0 ? BigInt(Date.now()) * 1_000_000n : BigInt(Math.round((performance.now() - start) * 1e6));
      view().setBigUint64(out, ns, true);
      return ESUCCESS;
    },
    fd_write(fd: number, iovs: number, iovsLen: number, nwritten: number) {
      const v = view();
      let total = 0;
      for (let i = 0; i < iovsLen; i++) {
        const ptr = v.getUint32(iovs + i * 8, true);
        const len = v.getUint32(iovs + i * 8 + 4, true);
        if (fd === 1 || fd === 2) pending[fd] += decoder.decode(bytes().subarray(ptr, ptr + len));
        total += len;
      }
      if (fd === 1 || fd === 2) {
        const lines = pending[fd]!.split("\n");
        pending[fd] = lines.pop()!;
        for (const line of lines) log(fd, line);
      }
      v.setUint32(nwritten, total, true);
      return fd === 1 || fd === 2 ? ESUCCESS : EBADF;
    },
    fd_fdstat_get(fd: number, out: number) {
      if (fd > 2) return EBADF;
      // A character device, no flags, every right.
      const v = view();
      v.setUint8(out, 2);
      v.setUint16(out + 2, 0, true);
      v.setBigUint64(out + 8, 0xffffffffffffffffn, true);
      v.setBigUint64(out + 16, 0xffffffffffffffffn, true);
      return ESUCCESS;
    },
    fd_prestat_get: unsupported,
    fd_prestat_dir_name: unsupported,
    fd_close: unsupported,
    fd_read: unsupported,
    fd_seek: unsupported,
    fd_sync: unsupported,
    fd_fdstat_set_flags: unsupported,
    fd_filestat_get: unsupported,
    fd_filestat_set_size: unsupported,
    path_create_directory: unsupported,
    path_filestat_get: unsupported,
    path_filestat_set_times: unsupported,
    path_open: unsupported,
    path_readlink: unsupported,
    path_remove_directory: unsupported,
    path_unlink_file: unsupported,
    poll_oneoff: () => ENOSYS,
    proc_exit(code: number) {
      throw new ProcExit(code);
    },
  };
}
