import { immutable } from './canonical.js';
const catalogReferenceBrand = Symbol('asha-rpg.catalog-reference');
const authoredCatalogOwnership = Symbol('asha-rpg.authored-catalog-ownership');
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
        references[name] = immutable({
            definitionId: entry.definitionId,
            category: entry.category,
            packageId: input.packageId,
            [catalogReferenceBrand]: true,
        });
    }
    definitions.sort((left, right) => left.id.localeCompare(right.id));
    return immutable({
        packageId: input.packageId,
        definitions: immutable(definitions),
        references: immutable(references),
    });
}
export function catalogDefinitionId(reference) {
    return typeof reference === 'string' ? reference : reference.definitionId;
}
/** @internal Retains authored owner identity on an AST node without serializing it. */
export function retainCatalogOwnership(value, fields) {
    const ownership = fields.flatMap(({ field, reference }) => isCatalogReference(reference)
        ? [
            immutable({
                field,
                definitionId: reference.definitionId,
                category: reference.category,
                packageId: reference.packageId,
            }),
        ]
        : []);
    if (ownership.length > 0) {
        Object.defineProperty(value, authoredCatalogOwnership, {
            value: immutable(ownership),
            enumerable: false,
            configurable: false,
            writable: false,
        });
    }
    return value;
}
/** @internal Reads owner identity retained by the typed authoring builders. */
export function catalogOwnershipOf(value) {
    if (!(authoredCatalogOwnership in value))
        return [];
    const ownership = value[authoredCatalogOwnership];
    return Array.isArray(ownership) ? ownership : [];
}
function isCatalogReference(value) {
    return (value !== null &&
        typeof value === 'object' &&
        catalogReferenceBrand in value);
}
function assertIdentifier(value, label) {
    if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value)) {
        throw new Error(`${label} must be a non-empty portable identifier`);
    }
}
//# sourceMappingURL=catalogs.js.map