const TEST_FILE_PATTERNS = [
  /\.test\.[mc]?[jt]sx?$/,
  /\.behavior\.test\.[jt]sx?$/,
  /\.render\.test\.[jt]sx?$/,
  /\.spec\.[jt]sx?$/,
  /(?:^|[/\\])tests?\.rs$/,
  /(?:^|[/\\])[^/\\]*_tests?\.rs$/,
  /(?:^|[/\\])test_[^/\\]+\.rs$/,
  /[/\\]tests[/\\]/,
  /[/\\]__tests__[/\\]/,
];

export function isTestSourceFile(relativePath) {
  return TEST_FILE_PATTERNS.some((pattern) => pattern.test(relativePath));
}
