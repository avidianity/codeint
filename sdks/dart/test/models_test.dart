import 'package:codeint/codeint.dart';
import 'package:test/test.dart';

void main() {
  test('CheckReport parses all 12 built-in metrics, diagnostics, and custom metrics',
      () {
    final report = CheckReport.fromJson(<String, Object?>{
      'files': 10,
      'source_lines': 400,
      'baseline': <String, Object?>{
        'path': '.codeint/baseline.json',
        'hidden_findings': 3,
      },
      'file_diagnostics': <Object?>[
        <String, Object?>{
          'path': 'lib/a.dart',
          'line': 4,
          'kind': 'parse_warning',
          'message': 'partial parse',
        },
      ],
      'scope_summaries': <Object?>[
        <String, Object?>{
          'language': 'dart',
          'package': 'sdks/dart',
          'dialect': 'dart',
          'kind': 'package_scope',
          'fields': <String, Object?>{'exports': '12'},
        },
      ],
      'source_sets': <String, Object?>{
        'production': <String, Object?>{
          'files': 8,
          'source_lines': 320,
          'erosion': <String, Object?>{
            'ratio': 0.05,
            'metric_version': 2,
            'reference_range': <String, Object?>{
              'low': 0.0,
              'high': 0.15,
              'source': 'dart',
            },
            'eroded_function_mass': 1.0,
            'total_function_mass': 20.0,
            'findings': const <Object?>[],
          },
          'cognitive_complexity': <String, Object?>{
            'ratio': 0.0,
            'metric_version': 1,
            'reference_range': 'default',
            'complex_cognitive_complexity': 0,
            'total_cognitive_complexity': 15,
            'findings': const <Object?>[],
          },
          'duplication': <String, Object?>{
            'ratio': 0.0,
            'metric_version': 2,
            'reference_range': 'default',
            'clone_lines': 0,
            'clone_groups': const <Object?>[],
          },
          'verbosity': <String, Object?>{
            'ratio': 0.0,
            'metric_version': 1,
            'reference_range': 'default',
            'verbose_lines': 0,
            'flagged_lines': 0,
            'findings': const <Object?>[],
          },
          'import_cycles': <String, Object?>{
            'ratio': 0.0,
            'metric_version': 1,
            'reference_range': 'default',
            'modules_in_import_cycles': 0,
            'modules': 8,
            'findings': const <Object?>[],
            'unresolved_imports': <String, Object?>{
              'count': 0,
              'imports': const <Object?>[],
            },
          },
          'boundaries': <String, Object?>{
            'ratio': 0.0,
            'metric_version': 2,
            'resolved_imports': 12,
            'findings': const <Object?>[],
          },
          'orphan_files': <String, Object?>{
            'ratio': 0.0,
            'metric_version': 1,
            'orphan_files': 0,
            'evaluated_files': 8,
            'entry_points': 2,
            'unreferenced': 0,
            'test_only': 0,
            'findings': const <Object?>[],
          },
          'test_gap': <String, Object?>{
            'ratio': 0.1,
            'metric_version': 1,
            'untested_function_mass': 2.0,
            'total_function_mass': 20.0,
            'measured_functions': 10,
            'findings': const <Object?>[],
          },
          'hazards': <String, Object?>{
            'ratio': 0.0,
            'metric_version': 1,
            'hazard_lines': 0,
            'source_lines': 320,
            'findings': const <Object?>[],
          },
          'custom_wasm_metric': <String, Object?>{
            'ratio': 0.25,
            'metric_version': 1,
            'reference_range': 'plugin',
            'numerator': 2.0,
            'denominator': 8.0,
            'findings': <Object?>[
              <String, Object?>{
                'id': 'f1',
                'path': 'lib/a.dart',
                'line': 10,
                'message': 'custom finding',
              },
            ],
          },
        },
      },
      'dependencies': <String, Object?>{
        'unused_dependencies': <String, Object?>{
          'ratio': 0.0,
          'metric_version': 1,
          'unused': 0,
          'declared': 2,
          'findings': const <Object?>[],
        },
        'unlisted_dependencies': <String, Object?>{
          'ratio': 0.0,
          'metric_version': 1,
          'unlisted': 0,
          'imported': 2,
          'findings': const <Object?>[],
        },
        'dev_in_production_dependencies': <String, Object?>{
          'ratio': 0.0,
          'metric_version': 1,
          'dev_in_production': 0,
          'production_imported': 2,
          'findings': const <Object?>[],
        },
      },
    });

    final prod = report.sourceSets['production']!;
    expect(prod.erosion.metricVersion, equals(2));
    expect(prod.erosion.referenceRange?.isRange, isTrue);
    expect(prod.cognitiveComplexity.metricVersion, equals(1));
    expect(prod.duplication.metricVersion, equals(2));
    expect(prod.verbosity, isA<VerbositySection>());
    expect(prod.verbosity.metricVersion, equals(1));
    expect(prod.verbosity.verboseLines, equals(0));
    expect(prod.verbosity.flaggedLines, equals(0));
    expect(prod.importCycles?.modules, equals(8));
    expect(prod.boundaries?.resolvedImports, equals(12));
    expect(prod.boundaries?.referenceRange, isNull);
    expect(prod.orphanFiles?.evaluatedFiles, equals(8));
    expect(prod.testGap?.measuredFunctions, equals(10));
    expect(prod.hazards, isA<HazardsSection>());
    expect(prod.hazards?.hazardLines, equals(0));
    expect(prod.hazards?.sourceLines, equals(320));
    expect(report.dependencies?.unusedDependencies.declared, equals(2));
    expect(
      report.dependencies?.unlistedDependencies,
      isA<UnlistedDependenciesSection>(),
    );
    expect(report.dependencies?.unlistedDependencies.unlisted, equals(0));
    expect(report.dependencies?.unlistedDependencies.imported, equals(2));
    expect(
      report.dependencies?.devInProductionDependencies,
      isA<DevInProductionDependenciesSection>(),
    );
    expect(
      report.dependencies?.devInProductionDependencies.devInProduction,
      equals(0),
    );
    expect(
      report.dependencies?.devInProductionDependencies.productionImported,
      equals(2),
    );
    expect(report.baseline?.hiddenFindings, equals(3));
    expect(report.fileDiagnostics.single.kind, equals('parse_warning'));
    expect(report.scopeSummaries.single.fields['exports'], equals('12'));
    expect(prod.externalMetrics['custom_wasm_metric']?.ratio, equals(0.25));
    expect(
      prod.externalMetrics['custom_wasm_metric']?.findings.single.message,
      equals('custom finding'),
    );
  });

  test('DiffReport and TrendReport parse custom external metrics and clone families',
      () {
    final diff = DiffReport.fromJson(<String, Object?>{
      'base': <String, Object?>{'ref': 'main', 'merge_base': 'abc123'},
      'head': <String, Object?>{'ref': 'feat', 'commit': 'def456'},
      'source_sets': <String, Object?>{
        'production': <String, Object?>{
          'erosion': _emptyChangeJson(2),
          'cognitive_complexity': _emptyChangeJson(1),
          'duplication': <String, Object?>{
            ..._emptyChangeJson(2),
            'clone_families': <String, Object?>{
              'new': <Object?>[
                <String, Object?>{
                  'id': 'cf-1',
                  'duplicated_lines': 12,
                  'clone_group_ids': const <String>['cg-1'],
                  'locations': <Object?>[
                    <String, Object?>{
                      'path': 'lib/a.dart',
                      'start_line': 1,
                      'end_line': 12,
                    },
                  ],
                },
              ],
              'new_left_out': 2,
              'resolved': const <Object?>[],
              'resolved_left_out': 1,
              'unchanged': 3,
              'unlisted_clone_family_group_ids': const <String>['cg-9'],
            },
          },
          'verbosity': _emptyChangeJson(1),
          'custom_plugin_metric': _emptyChangeJson(1),
        },
      },
    });

    final prodChanges = diff.sourceSets['production']!;
    expect(diff.head.commit, equals('def456'));
    expect(prodChanges.duplication, isA<DuplicationChanges>());
    final families = prodChanges.duplication.cloneFamilies!;
    expect(families.added.single.id, equals('cf-1'));
    expect(families.newLeftOut, equals(2));
    expect(families.resolved, isEmpty);
    expect(families.resolvedLeftOut, equals(1));
    expect(families.unchanged, equals(3));
    expect(families.unlistedCloneFamilyGroupIds, equals(<String>['cg-9']));
    expect(
      prodChanges.externalMetrics.containsKey('custom_plugin_metric'),
      isTrue,
    );

    final trend = TrendReport.fromJson(<String, Object?>{
      'commits': 1,
      'source_sets': <String, Object?>{
        'production': <Object?>[
          <String, Object?>{
            'commit': 'abc123',
            'committed_on': '2026-11-28',
            'files': 5,
            'erosion': 0.0,
            'cognitive_complexity': 0.0,
            'duplication': 0.0,
            'verbosity': 0.0,
            'custom_plugin_metric': 0.12,
          },
        ],
      },
    });
    expect(
      trend.sourceSets['production']!.single.externalMetrics['custom_plugin_metric'],
      equals(0.12),
    );
  });
}

Map<String, Object?> _emptyChangeJson(int metricVersion) => <String, Object?>{
      'base_ratio': 0.0,
      'head_ratio': 0.0,
      'metric_version': metricVersion,
      'new': const <Object?>[],
      'resolved': const <Object?>[],
      'unchanged': 0,
    };
