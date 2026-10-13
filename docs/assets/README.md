# Assets Directory

This directory contains visual assets referenced throughout the documentation.

## Structure

- **`architecture/`** - System architecture diagrams, component interactions, and data flow visualizations
- **`ui-flows/`** - User interface screenshots, wireframes, and interaction flows from Figma
- **`data-models/`** - Entity relationship diagrams, database schemas, and data structure visualizations

## Adding Assets

### From Figma
1. Open the [Always Together Figma file](https://www.figma.com/design/BqFYN5OtLLti55TAtB8xPX/Untitled)
2. Select the frame or component you want to export
3. In the right sidebar, click "Export"
4. Choose PNG or SVG format (SVG preferred for diagrams, PNG for UI screens)
5. Save to the appropriate subdirectory with a descriptive name
6. Reference in markdown using: `![Description](../assets/subdir/filename.png)`

### Naming Convention
- Use lowercase with hyphens: `location-sharing-flow.png`
- Include version/date if iterating: `auth-flow-v2-2024-09.png`
- Be descriptive but concise

## Current Assets

*No assets currently stored in repository.* 

**Note:** Due to repository size constraints, consider:
- Using SVG for diagrams where possible
- Compressing PNGs with tools like `pngquant` or `tinypng`
- Linking to external hosting (e.g., GitHub Releases, CDN) for large image sets
- Keeping only essential diagrams in-repo; link to Figma for comprehensive views

## Referencing in Documentation

```markdown
![Location Sharing Flow](../assets/ui-flows/location-sharing-sequence.png)
*Figure 1: Sequence diagram showing the location sharing initiation flow.*
```

Always include:
- Alt text describing the image content
- A figure caption below the image
- Context explaining what the reader should notice
