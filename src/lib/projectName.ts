export function projectName(cwd: string) {
  if (/^[A-Za-z]:[\\/]+$/.test(cwd)) return cwd;
  const withoutTrailingSeparators = cwd.replace(/[\\/]+$/, "");
  return withoutTrailingSeparators.split(/[\\/]/).pop() || cwd;
}
