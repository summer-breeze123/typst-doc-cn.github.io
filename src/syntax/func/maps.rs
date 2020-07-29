//! Deduplicating maps and keys for argument parsing.

use crate::diagnostic::Diagnostics;
use crate::layout::prelude::*;
use crate::length::{ScaleLength, Value4};
use crate::syntax::span::Spanned;
use super::keys::*;
use super::values::*;
use super::*;

/// A map which deduplicates redundant arguments.
///
/// Whenever a duplicate argument is inserted into the map, through the
/// functions `from_iter`, `insert` or `extend` an diagnostics is added to the error
/// list that needs to be passed to those functions.
///
/// All entries need to have span information to enable the error reporting.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct DedupMap<K, V> where K: Eq {
    map: Vec<Spanned<(K, V)>>,
}

impl<K, V> DedupMap<K, V> where K: Eq {
    /// Create a new deduplicating map.
    pub fn new() -> DedupMap<K, V> {
        DedupMap { map: vec![] }
    }

    /// Create a new map from an iterator of spanned keys and values.
    pub fn from_iter<I>(diagnostics: &mut Diagnostics, iter: I) -> DedupMap<K, V>
    where I: IntoIterator<Item=Spanned<(K, V)>> {
        let mut map = DedupMap::new();
        map.extend(diagnostics, iter);
        map
    }

    /// Add a spanned key-value pair.
    pub fn insert(&mut self, diagnostics: &mut Diagnostics, entry: Spanned<(K, V)>) {
        if self.map.iter().any(|e| e.v.0 == entry.v.0) {
            diagnostics.push(error!(entry.span, "duplicate argument"));
        } else {
            self.map.push(entry);
        }
    }

    /// Add multiple spanned key-value pairs.
    pub fn extend<I>(&mut self, diagnostics: &mut Diagnostics, items: I)
    where I: IntoIterator<Item=Spanned<(K, V)>> {
        for item in items.into_iter() {
            self.insert(diagnostics, item);
        }
    }

    /// Get the value corresponding to a key if it is present.
    pub fn get(&self, key: K) -> Option<&V> {
        self.map.iter().find(|e| e.v.0 == key).map(|e| &e.v.1)
    }

    /// Get the value and its span corresponding to a key if it is present.
    pub fn get_spanned(&self, key: K) -> Option<Spanned<&V>> {
        self.map.iter().find(|e| e.v.0 == key)
            .map(|e| Spanned { v: &e.v.1, span: e.span })
    }

    /// Call a function with the value if the key is present.
    pub fn with<F>(&self, key: K, callback: F) where F: FnOnce(&V) {
        if let Some(value) = self.get(key) {
            callback(value);
        }
    }

    /// Create a new map where keys and values are mapped to new keys and
    /// values. When the mapping introduces new duplicates, diagnostics are
    /// generated.
    pub fn dedup<F, K2, V2>(&self, diagnostics: &mut Diagnostics, mut f: F) -> DedupMap<K2, V2>
    where F: FnMut(&K, &V) -> (K2, V2), K2: Eq {
        let mut map = DedupMap::new();

        for Spanned { v: (key, value), span } in self.map.iter() {
            let (key, value) = f(key, value);
            map.insert(diagnostics, Spanned { v: (key, value), span: *span });
        }

        map
    }

    /// Iterate over the (key, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item=&(K, V)> {
        self.map.iter().map(|e| &e.v)
    }
}

/// A map for storing a value for axes given by keyword arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMap<V>(DedupMap<AxisKey, V>);

impl<V: Value> AxisMap<V> {
    /// Parse an axis map from the object.
    pub fn parse<K>(
        diagnostics: &mut Diagnostics,
        object: &mut Object,
    ) -> AxisMap<V> where K: Key + Into<AxisKey> {
        let values: Vec<_> = object
            .get_all_spanned::<K, V>(diagnostics)
            .map(|s| s.map(|(k, v)| (k.into(), v)))
            .collect();

        AxisMap(DedupMap::from_iter(diagnostics, values))
    }

    /// Deduplicate from specific or generic to just specific axes.
    pub fn dedup(&self, diagnostics: &mut Diagnostics, axes: LayoutAxes) -> DedupMap<SpecificAxis, V>
    where V: Clone {
        self.0.dedup(diagnostics, |key, val| (key.to_specific(axes), val.clone()))
    }
}

/// A map for storing values for axes that are given through a combination of
/// (two) positional and keyword arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct PosAxisMap<V>(DedupMap<PosAxisKey, V>);

impl<V: Value> PosAxisMap<V> {
    /// Parse a positional/axis map from the function arguments.
    pub fn parse<K>(
        diagnostics: &mut Diagnostics,
        args: &mut FuncArgs,
    ) -> PosAxisMap<V> where K: Key + Into<AxisKey> {
        let mut map = DedupMap::new();

        for &key in &[PosAxisKey::First, PosAxisKey::Second] {
            if let Some(Spanned { v, span }) = args.pos.get::<Spanned<V>>(diagnostics) {
                map.insert(diagnostics, Spanned { v: (key, v), span })
            }
        }

        let keywords: Vec<_> = args.key
            .get_all_spanned::<K, V>(diagnostics)
            .map(|s| s.map(|(k, v)| (PosAxisKey::Keyword(k.into()), v)))
            .collect();

        map.extend(diagnostics, keywords);

        PosAxisMap(map)
    }

    /// Deduplicate from positional arguments and keyword arguments for generic
    /// or specific axes to just generic axes.
    pub fn dedup<F>(
        &self,
        diagnostics: &mut Diagnostics,
        axes: LayoutAxes,
        mut f: F,
    ) -> DedupMap<GenericAxis, V>
    where
        F: FnMut(&V) -> Option<GenericAxis>,
        V: Clone,
    {
        self.0.dedup(diagnostics, |key, val| {
            (match key {
                PosAxisKey::First => f(val).unwrap_or(GenericAxis::Primary),
                PosAxisKey::Second => f(val).unwrap_or(GenericAxis::Secondary),
                PosAxisKey::Keyword(AxisKey::Specific(axis)) => axis.to_generic(axes),
                PosAxisKey::Keyword(AxisKey::Generic(axis)) => *axis,
            }, val.clone())
        })
    }
}

/// A map for storing padding given for a combination of all sides, opposing
/// sides or single sides.
#[derive(Debug, Clone, PartialEq)]
pub struct PaddingMap(DedupMap<PaddingKey<AxisKey>, Option<ScaleLength>>);

impl PaddingMap {
    /// Parse a padding map from the function arguments.
    pub fn parse(diagnostics: &mut Diagnostics, args: &mut FuncArgs) -> PaddingMap {
        let mut map = DedupMap::new();

        let all = args.pos.get::<Spanned<Defaultable<ScaleLength>>>(diagnostics);
        if let Some(Spanned { v, span }) = all {
            map.insert(diagnostics, Spanned { v: (PaddingKey::All, v.into()), span });
        }

        let paddings: Vec<_> = args.key
            .get_all_spanned::<PaddingKey<AxisKey>, Defaultable<ScaleLength>>(diagnostics)
            .map(|s| s.map(|(k, v)| (k, v.into())))
            .collect();

        map.extend(diagnostics, paddings);

        PaddingMap(map)
    }

    /// Apply the specified padding on a value box of optional, scalable sizes.
    pub fn apply(
        &self,
        diagnostics: &mut Diagnostics,
        axes: LayoutAxes,
        padding: &mut Value4<Option<ScaleLength>>
    ) {
        use PaddingKey::*;

        let map = self.0.dedup(diagnostics, |key, &val| {
            (match key {
                All => All,
                Both(axis) => Both(axis.to_specific(axes)),
                Side(axis, alignment) => {
                    let generic = axis.to_generic(axes);
                    let axis = axis.to_specific(axes);
                    Side(axis, alignment.to_specific(axes, generic))
                }
            }, val)
        });

        map.with(All, |&val| padding.set_all(val));
        map.with(Both(Horizontal), |&val| padding.set_horizontal(val));
        map.with(Both(Vertical), |&val| padding.set_vertical(val));

        for &(key, val) in map.iter() {
            if let Side(_, alignment) = key {
                match alignment {
                    AlignmentValue::Left => padding.left = val,
                    AlignmentValue::Right => padding.right = val,
                    AlignmentValue::Top => padding.top = val,
                    AlignmentValue::Bottom => padding.bottom = val,
                    _ => {},
                }
            }
        }
    }
}
