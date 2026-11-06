// Per-browser memory of the user's ROMs and disks (usability plan U6).
// IndexedDB, this origin only; nothing is sent anywhere. Every call is
// best-effort: a private window or blocked storage just means files must be
// chosen again.
const DB = 'ibm_pc_5150_media';
const STORE = 'slots';

function open() {
  return new Promise((resolve, reject) => {
    let req;
    try { req = indexedDB.open(DB, 1); } catch (e) { reject(e); return; }
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function tx(mode, fn) {
  const db = await open();
  return new Promise((resolve, reject) => {
    const t = db.transaction(STORE, mode);
    const result = fn(t.objectStore(STORE));
    t.oncomplete = () => { db.close(); resolve(result && result.result); };
    t.onerror = () => { db.close(); reject(t.error); };
  });
}

export async function remember(slot, name, bytes) {
  try { await tx('readwrite', (s) => s.put({ name, bytes }, slot)); } catch { /* storage unavailable */ }
}

export async function forget(slot) {
  try { await tx('readwrite', (s) => s.delete(slot)); } catch { /* storage unavailable */ }
}

export async function recall() {
  const out = [];
  try {
    const db = await open();
    await new Promise((resolve, reject) => {
      const t = db.transaction(STORE, 'readonly');
      const req = t.objectStore(STORE).openCursor();
      req.onsuccess = () => {
        const c = req.result;
        if (c) { out.push({ slot: Number(c.key), name: c.value.name, bytes: c.value.bytes }); c.continue(); }
      };
      t.oncomplete = () => { db.close(); resolve(); };
      t.onerror = () => { db.close(); reject(t.error); };
    });
  } catch { /* storage unavailable */ }
  return out.sort((a, b) => a.slot - b.slot);
}
