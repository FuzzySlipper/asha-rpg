import { immutable } from './canonical.js';
import { catalogDefinitionId } from './catalogs.js';
export function patchParameter(id) {
    return immutable({ parameter: id });
}
export function combineRulesetPatches(...patches) {
    return patch(patches.flatMap((entry) => entry.operations));
}
export const actionPatch = immutable({
    semantic: immutable({
        maximumRange: numberField('semantic', ['targets', 'maximumRange']),
        maximumTargets: numberField('semantic', ['targets', 'maximumTargets']),
        cost(resource) {
            const member = {
                kind: 'member',
                key: 'resourceId',
                value: catalogDefinitionId(resource),
            };
            return immutable({
                amount: numberField('semantic', ['costs', member, 'amount']),
                remove() {
                    return patch([
                        {
                            kind: 'removeMember',
                            plane: 'semantic',
                            path: [field('costs')],
                            identity: member,
                        },
                    ]);
                },
            });
        },
    }),
    presentation: immutable({
        label: scalarField('presentation', ['label']),
        description: upsertScalarField('presentation', ['description']),
    }),
});
function numberField(plane, path) {
    return immutable({
        set(value) {
            return patch([
                { kind: 'setScalar', plane, path: segments(path), value },
            ]);
        },
        adjust(options) {
            return patch([
                {
                    kind: 'adjustNumber',
                    plane,
                    path: segments(path),
                    multiply: options.multiply ?? 1,
                    add: options.add ?? 0,
                },
            ]);
        },
    });
}
function scalarField(plane, path) {
    return immutable({
        set(value) {
            return patch([
                { kind: 'setScalar', plane, path: segments(path), value },
            ]);
        },
    });
}
function upsertScalarField(plane, path) {
    return immutable({
        set(value) {
            return patch([
                { kind: 'upsertScalar', plane, path: segments(path), value },
            ]);
        },
    });
}
function patch(operations) {
    return immutable({ version: 1, operations: [...operations] });
}
function segments(values) {
    return values.map((value) => typeof value === 'string' ? field(value) : value);
}
function field(name) {
    return { kind: 'field', name };
}
//# sourceMappingURL=ruleset-patch-builders.js.map