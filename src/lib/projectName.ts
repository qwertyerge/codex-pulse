export function projectName(cwd: string) {
  const withoutTrailingSeparators = cwd.replace(/[\\/]+$/, "");
  return withoutTrailingSeparators.split(/[\\/]/).pop() || cwd;
}
