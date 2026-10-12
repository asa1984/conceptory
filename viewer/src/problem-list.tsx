import type { Diagnostic } from "./data.ts";
import { href } from "./route.ts";

const noLinks: ReadonlySet<string> = new Set();

type Props = {
  problems: Diagnostic[];
  /** Paths in this set are rendered as links to their documents. */
  linkable?: ReadonlySet<string>;
};

export const ProblemList = ({ problems, linkable = noLinks }: Props) => (
  <ul className="space-y-1 font-mono text-xs">
    {problems.map((problem) => (
      <li key={`${problem.path}:${problem.line.toString()}:${problem.rule}:${problem.message}`}>
        {linkable.has(problem.path) ? (
          <a className="text-accent underline" href={href({ view: "document", id: problem.path })}>
            {problem.path}
          </a>
        ) : (
          <span>{problem.path}</span>
        )}
        :{problem.line} {problem.message} <span className="text-fg-muted">[{problem.rule}]</span>
      </li>
    ))}
  </ul>
);
