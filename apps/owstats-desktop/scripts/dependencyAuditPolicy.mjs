export function validateSecurityPolicy(policy, now = Date.now()) {
  if (policy.schemaVersion !== 1) throw new Error("Unsupported dependency security policy schema");

  const exception = policy.reactRouterException;
  if (!exception || typeof exception !== "object") throw new Error("Missing React Router security exception");
  const expiresAt = Date.parse(`${exception.expiresOn}T23:59:59.999Z`);
  if (!Number.isFinite(expiresAt)) throw new Error("Invalid React Router security exception expiry date");
  if (now > expiresAt) {
    throw new Error(`React Router security exception ${exception.id} expired on ${exception.expiresOn}; manual review is required`);
  }

  const allowedPackages = new Set(Object.keys(exception.exactVersions ?? {}));
  const allowedAdvisories = new Set(exception.allowedAdvisoryUrls ?? []);
  if (allowedPackages.size === 0 || allowedAdvisories.size === 0) {
    throw new Error("React Router security exception must declare packages and advisory URLs");
  }
  return { exception, allowedPackages, allowedAdvisories };
}

export function validateAuditReport(report, auditStatus, policy, now = Date.now()) {
  const { exception, allowedPackages, allowedAdvisories } = validateSecurityPolicy(policy, now);
  const vulnerabilities = Object.entries(report.vulnerabilities ?? {});

  if (vulnerabilities.length === 0) {
    if (auditStatus !== 0) throw new Error(`npm audit reported no vulnerabilities but exited with ${auditStatus}`);
    return { mode: "clean", exception };
  }

  if (auditStatus !== 1) {
    throw new Error(`npm audit returned unexpected exit code ${auditStatus} while reporting vulnerabilities`);
  }

  const reportedAdvisories = new Set();
  for (const [name, finding] of vulnerabilities) {
    if (!allowedPackages.has(name)) throw new Error(`Unapproved vulnerable production package: ${name}`);
    if (!Array.isArray(finding.via) || finding.via.length === 0) {
      throw new Error(`Malformed npm audit finding for ${name}`);
    }
    for (const via of finding.via) {
      if (typeof via === "string") {
        if (!allowedPackages.has(via)) throw new Error(`Unapproved transitive vulnerability chain: ${name} via ${via}`);
        continue;
      }
      if (!via || typeof via.url !== "string" || !allowedAdvisories.has(via.url)) {
        throw new Error(`Advisory set changed for ${name}; manual review is required`);
      }
      reportedAdvisories.add(via.url);
    }
  }

  const reportedPackages = new Set(vulnerabilities.map(([name]) => name));
  for (const dependency of allowedPackages) {
    if (!reportedPackages.has(dependency)) {
      throw new Error(`React Router exception shape changed: expected audit finding for ${dependency}`);
    }
  }
  for (const advisory of allowedAdvisories) {
    if (!reportedAdvisories.has(advisory)) {
      throw new Error(`React Router exception shape changed: expected advisory ${advisory}`);
    }
  }
  return { mode: "exception", exception };
}
