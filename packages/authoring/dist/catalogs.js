import { immutable } from './canonical.js';
export function defineRulesetCatalog(input) {
    assertIdentifier(input.packageId, 'catalog package id');
    if (input.sourceModule.length === 0) {
        throw new Error('catalog source module must not be empty');
    }
    const definitions = [];
    const references = {};
    for (const [name, entry] of Object.entries(input.entries)) {
        assertIdentifier(name, 'catalog entry name');
        assertIdentifier(entry.definitionId, 'catalog definition id');
        assertIdentifier(entry.id, 'catalog semantic id');
        if (entry.label.length === 0)
            throw new Error('catalog label must not be empty');
        definitions.push(immutable({
            kind: 'support',
            id: entry.definitionId,
            visibility: 'public',
            extensionPolicy: 'sealed',
            source: {
                module: input.sourceModule,
                declaration: name,
            },
            presentation: {
                label: entry.label,
                ...(entry.description === undefined
                    ? {}
                    : { description: entry.description }),
                ...(entry.tags === undefined ? {} : { tags: [...entry.tags] }),
            },
            semantic: { catalog: entry.category, id: entry.id },
        }));
        references[name] = entry.definitionId;
    }
    definitions.sort((left, right) => left.id.localeCompare(right.id));
    return immutable({
        packageId: input.packageId,
        definitions: immutable(definitions),
        references: immutable(references),
    });
}
function assertIdentifier(value, label) {
    if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value)) {
        throw new Error(`${label} must be a non-empty portable identifier`);
    }
}
//# sourceMappingURL=catalogs.js.map