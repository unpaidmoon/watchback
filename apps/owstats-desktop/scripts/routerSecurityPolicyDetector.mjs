const executableExtension = /\.(?:[cm]?[jt]sx?)$/i;

const moduleReferencePatterns = [
  /\b(?:import|export)\s+(?:type\s+)?(?:[^;\n]*?\s+from\s*)?["']([^"']+)["']/g,
  /\bimport\s*\(\s*["']([^"']+)["']\s*\)/g,
  /\brequire\s*\(\s*["']([^"']+)["']\s*\)/g,
];

function isForbiddenModule(specifier, forbiddenRoots) {
  return forbiddenRoots.some((root) => specifier === root || specifier.startsWith(`${root}/`));
}

export function detectRouterSecurityViolations(source, boundary) {
  const violations = [];

  for (const pattern of moduleReferencePatterns) {
    pattern.lastIndex = 0;
    for (const match of source.matchAll(pattern)) {
      if (isForbiddenModule(match[1], boundary.forbiddenModuleSpecifiers)) {
        violations.push(`forbidden module reference: ${match[1]}`);
      }
    }
  }

  for (const identifier of boundary.forbiddenIdentifiers) {
    const escaped = identifier.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const pattern = identifier.includes(".")
      ? new RegExp(`(^|[^\\w$])${escaped}(?![\\w$])`)
      : new RegExp(`\\b${escaped}\\b`);
    if (pattern.test(source)) violations.push(`forbidden Router API: ${identifier}`);
  }

  return [...new Set(violations)];
}

export function isExecutableSourceFile(fileName) {
  return executableExtension.test(fileName);
}

export function isRootBuildConfig(fileName) {
  return /^(?:vite|vitest|webpack|rollup|tsup|playwright|eslint|postcss|tailwind)\.config\.(?:[cm]?[jt]s)$/i.test(fileName);
}
