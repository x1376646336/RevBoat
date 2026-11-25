export interface RemoteEntry { name: string; kind: string; revision: number; author: string; date: string }
export function parseRemoteEntries(xml: string): RemoteEntry[] {
  const document = new DOMParser().parseFromString(xml, 'application/xml')
  if (document.querySelector('parsererror') || !document.querySelector('lists')) throw new Error('Invalid SVN directory response')
  return Array.from(document.querySelectorAll('list > entry')).map(entry => ({
    name: entry.querySelector('name')?.textContent || '',
    kind: entry.getAttribute('kind') || 'file',
    revision: Number(entry.querySelector('commit')?.getAttribute('revision') || 0),
    author: entry.querySelector('author')?.textContent || '',
    date: entry.querySelector('date')?.textContent || '',
  })).sort((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name, undefined, { numeric: true }) : a.kind === 'dir' ? -1 : 1))
}
export function childRepositoryUrl(parent: string, name: string): string {
  if (!name || name === '.' || name === '..' || /[\\/]/.test(name)) throw new Error('Invalid repository entry name')
  return `${parent.replace(/\/+$/, '')}/${encodeURIComponent(name)}`
}
