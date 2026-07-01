# Code Review and Merge Workflow

When executing implementation plans or using subagent-driven-development:
1. Ensure the development happens on a dedicated feature branch.
2. Utilize automated reviewer sub-agents to verify spec compliance and code quality.
3. When implementation is complete, do **NOT** merge the branch or create a Pull Request.
4. Instead, hand the branch over to the user so they can review the code locally, run tests, and merge it manually into main before pushing to GitHub.


# Temporary Files and Tools
When creating temporary scripts or tools to aid in debugging, testing, or code exploration, always place them in a dedicated folder such as `temp/tools` within the project root instead of cluttering the root directory.
