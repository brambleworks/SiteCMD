import {
  chownSync,
  closeSync,
  constants,
  fstatSync,
  openSync,
  readSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";

export const requestLimit = 1024 * 1024;
export const responseLimit = 8 * requestLimit;
export const bridgeTimeoutMs = 150000;

export function readMessage(file, limit, uid) {
  const fd = openSync(file, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const before = fstatSync(fd);
    if (!before.isFile() || before.nlink !== 1 || (uid !== undefined && before.uid !== uid))
      throw new Error("Bridge messages must be owned regular files without links");
    if (before.size > limit) throw new Error("Bridge message exceeds the byte limit");
    const bytes = Buffer.alloc(before.size + 1);
    let size = 0;
    while (size < bytes.length) {
      const count = readSync(fd, bytes, size, bytes.length - size, null);
      if (!count) break;
      size += count;
    }
    const after = fstatSync(fd);
    if (size > limit) throw new Error("Bridge message exceeds the byte limit");
    if (
      before.size !== size ||
      after.size !== size ||
      before.mtimeMs !== after.mtimeMs ||
      after.nlink !== 1
    )
      throw new Error("Bridge message changed while reading");
    return JSON.parse(bytes.subarray(0, size).toString("utf8"));
  } finally {
    closeSync(fd);
  }
}

export function removeMessage(file) {
  try {
    unlinkSync(file);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
}

export function publishMessage(file, value, limit, mode, owner) {
  const bytes = JSON.stringify(value);
  if (Buffer.byteLength(bytes) > limit) throw new Error("Bridge message exceeds the byte limit");
  const temporary = `${file}.tmp`;
  writeFileSync(temporary, bytes, { flag: "wx", mode });
  try {
    if (owner) chownSync(temporary, owner.uid, owner.gid);
    renameSync(temporary, file);
  } finally {
    removeMessage(temporary);
  }
}
