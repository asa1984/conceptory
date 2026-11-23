// Types for the textlint rule packages that ship without declarations. Each declares the shape of
// module.exports, which is what a default import of a CommonJS module binds to.

declare module "textlint-rule-preset-ja-technical-writing" {
  const preset: {
    rules: Record<string, unknown>;
    rulesConfig: Record<string, unknown>;
  };
  export = preset;
}

declare module "@textlint-ja/textlint-rule-preset-ai-writing" {
  const module: {
    default: {
      rules: Record<string, unknown>;
      rulesConfig: Record<string, unknown>;
    };
  };
  export = module;
}

declare module "textlint-rule-prh" {
  const module: { default: unknown };
  export = module;
}
