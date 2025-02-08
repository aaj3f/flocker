# Code Reorganization Plan

To improve code organization and make it easier for developers to contribute, we should reorganize the codebase to better separate concerns. Here's the proposed restructuring:

## 1. CLI Module Restructuring

Current issues in `cli.rs`:

- Mixes command argument parsing with UI logic
- Handles Docker Hub API interactions
- Contains both container and ledger action management
- Combines UI state with business logic

### Proposed Structure

```
src/
├── cli/
│   ├── mod.rs           # Re-exports
│   ├── args.rs          # Command line argument parsing
│   ├── actions/
│   │   ├── mod.rs       # Action enums re-exports
│   │   ├── container.rs # Container actions
│   │   └── ledger.rs    # Ledger actions
│   ├── hub/
│   │   ├── mod.rs       # Docker Hub interactions
│   │   ├── tag.rs       # Tag handling
│   │   └── api.rs       # API client
│   └── ui.rs            # CLI UI state and interactions
```

### Component Responsibilities

1. `args.rs`:

   - Command line argument parsing via clap
   - Argument validation
   - Help text and documentation

2. `actions/container.rs`:

   - Container action enum and implementations
   - Container status display formatting
   - Container-specific user prompts

3. `actions/ledger.rs`:

   - Ledger action enum and implementations
   - Ledger display formatting
   - Ledger-specific user prompts

4. `hub/tag.rs`:

   - Tag struct and implementations
   - Tag formatting and display
   - Time formatting utilities

5. `hub/api.rs`:

   - Docker Hub API client
   - Tag fetching and pagination
   - API response handling

6. `ui.rs`:
   - CLI state management
   - User interaction flows
   - Theme and styling

## 2. Docker Module Improvements

Current `docker/` structure is good but could be enhanced:

```
src/docker/
├── mod.rs
├── manager.rs
└── types.rs
```

Add:

```
src/docker/
├── api/
│   ├── mod.rs    # API operations
│   ├── container.rs
│   ├── image.rs
│   └── ledger.rs
├── mod.rs
├── manager.rs
└── types.rs
```

This would separate Docker API operations by domain, making it easier to:

- Add new container operations
- Add new image operations
- Add new ledger operations

## 3. UI Module Enhancements

Current `ui/` structure:

```
src/ui/
├── mod.rs
├── container.rs
├── image.rs
└── ledger.rs
```

Add:

```
src/ui/
├── components/
│   ├── mod.rs
│   ├── prompt.rs    # Common prompt components
│   ├── select.rs    # Selection components
│   └── display.rs   # Display formatting
├── mod.rs
├── container.rs
├── image.rs
└── ledger.rs
```

This would:

- Reduce code duplication in UI components
- Make it easier to maintain consistent UI patterns
- Simplify adding new UI components

## 4. State Management Improvements

Split `state.rs` into:

```
src/state/
├── mod.rs
├── types.rs        # State-related types
├── persistence.rs  # State loading/saving
└── manager.rs      # State operations
```

Benefits:

- Clearer separation between types and operations
- Easier to modify persistence mechanism
- More focused testing

## Implementation Plan

1. Create new directory structure
2. Move existing code to new locations
3. Update imports and dependencies
4. Add new abstraction layers where needed
5. Update tests to match new structure
6. Update documentation

## Benefits

This reorganization will:

1. Make the codebase more modular and maintainable
2. Make it easier for new developers to understand and contribute
3. Improve testability by having more focused components
4. Make it easier to add new features by having clear extension points
5. Reduce cognitive load when working on specific features

## Migration Strategy

1. Create new structure alongside existing code
2. Gradually move functionality to new locations
3. Update one component at a time
4. Maintain test coverage throughout
5. Update documentation as we go

This allows for incremental improvements while keeping the application functional throughout the process.
