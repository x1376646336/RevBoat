// SVN log paths are decoded repository paths; encode each segment before
// appending a peg revision so #, %, ?, spaces and @ stay part of the filename.
export function repositoryDiffTarget(root: string, path: string, revision: number, action: string): string {
  const encodedPath = path.replace(/^\/+/, '').split('/').map(encodeURIComponent).join('/')
  const peg = action === 'D' ? Math.max(0, revision - 1) : revision
  return `${root.replace(/\/+$/, '')}/${encodedPath}@${peg}`
}
export function isRepositoryDiffTarget(path: string): boolean {
  return /^(?:https?|svn|svn\+ssh|file):\/\//i.test(path)
}
