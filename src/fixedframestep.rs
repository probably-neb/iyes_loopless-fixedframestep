//! Fixed Framestep implementation as a Bevy Stage
//!
//! This is an alternative to Bevy's FixedFramestep. It does not (ab)use run criteria; instead,
//! it runs in a dedicated stage, separate from your regular update systems. It does not conflict
//! with any other functionality, and can be combined with states, run conditions, etc.
//!
//! It is possible to add multiple "sub-stages" within a fixed framestep, allowing
//! you to apply `Commands` within a single framestep run. For example, if you want
//! to spawn entities and then do something with them, on the same tick.
//!
//! It is also possible to have multiple independent fixed framesteps, should you need to.
//!
//! (see `examples/fixedframestep.rs` to learn how to use it)
//!
//! Every frame, the [`FixedFramestepStage`] will accumulate the frame delta. When
//! it goes over the set framestep value, it will run all the child stages. It
//! will repeat the sequence of child stages multiple frames if needed, if
//! more than one framestep has accumulated.
//!
//! You can use the [`FixedFramesteps`] resource (make sure it is the one from this
//! crate, not the one from Bevy with the same name) to access information about a
//! fixed framestep and to control its parameters, like the framestep duration.

use bevy_utils::HashMap;

use bevy_ecs::prelude::*;

/// The "name" of a fixed framestep. Used to manipulate it.
pub type FramestepName = &'static str;

/// Not to be confused with bevy_core's `FrameCount`
pub type FrameCounter = u32;

/// Resource type that allows you to get info about and to manipulate fixed framestep state
///
/// If you want to access parameters of your fixed framestep(s), such as the framestep duration,
/// accumulator, and paused state, you can get them from this resource. They are contained
/// in a [`FixedFramestepInfo`] struct, which you can get using the various methods on this type.
///
/// If you mutate the framestep duration or paused state, they will be taken into account
/// from the next run of that fixed framestep.
///
/// From within a fixed framestep system, you can also mutate the accumulator. May be useful
/// for networking or other use cases that need to stretch frame.
#[derive(Default)]
#[derive(Resource)]
pub struct FixedFramesteps {
    info: HashMap<FramestepName, FixedFramestepInfo>,
    current: Option<FramestepName>,
}

impl FixedFramesteps {
    /// Returns a reference to the framestep info for a given framestep by name.
    pub fn get(&self, label: FramestepName) -> Option<&FixedFramestepInfo> {
        self.info.get(label)
    }

    /// Returns a reference to the framestep info for the currently running stage.
    ///
    /// Returns [`Some`] only if called inside a fixed framestep stage.
    pub fn get_current(&self) -> Option<&FixedFramestepInfo> {
        self.current.as_ref().and_then(|label| self.info.get(label))
    }

    /// Panicking version of [`get_current`]
    pub fn current(&self) -> &FixedFramestepInfo {
        self.get_current()
            .expect("FixedFramesteps::current can only be used when running inside a fixed framestep.")
    }

    /// Returns a reference to the framestep info, assuming you only have one.
    pub fn get_single(&self) -> Option<&FixedFramestepInfo> {
        if self.info.len() != 1 {
            return None;
        }
        self.info.values().next()
    }

    /// Panicking version of [`get_single`]
    pub fn single(&self) -> &FixedFramestepInfo {
        self.get_single().expect("Expected exactly one fixed framestep.")
    }

    /// Returns a mut reference to the framestep info for a given framestep by name.
    pub fn get_mut(&mut self, label: FramestepName) -> Option<&mut FixedFramestepInfo> {
        self.info.get_mut(label)
    }

    /// Returns a mut reference to the framestep info for the currently running stage.
    ///
    /// Returns [`Some`] only if called inside a fixed framestep stage.
    pub fn get_current_mut(&mut self) -> Option<&mut FixedFramestepInfo> {
        self.current.as_ref().and_then(|label| self.info.get_mut(label))
    }

    /// Panicking version of [`get_current_mut`]
    pub fn current_mut(&mut self) -> &mut FixedFramestepInfo {
        self.get_current_mut()
            .expect("FixedFramesteps::current can only be used when running inside a fixed framestep.")
    }

    /// Returns a mut reference to the framestep info, assuming you only have one.
    pub fn get_single_mut(&mut self) -> Option<&mut FixedFramestepInfo> {
        if self.info.len() != 1 {
            return None;
        }
        self.info.values_mut().next()
    }

    /// Panicking version of [`get_single_mut`]
    pub fn single_mut(&mut self) -> &mut FixedFramestepInfo {
        self.get_single_mut().expect("Expected exactly one fixed framestep.")
    }
}

/// Provides access to the parameters of a fixed framestep
///
/// You can get this using the [`FixedFramesteps`] resource.
pub struct FixedFramestepInfo {
    /// FrameCounter of each fixed framestep tick
    pub step: FrameCounter,
    /// Accumulated frame since the last fixed framestep run
    pub accumulator: FrameCounter,
    /// Is the fixed framestep paused?
    pub paused: bool,
}

impl FixedFramestepInfo {
    /// The frame duration of each framestep
    pub fn framestep(&self) -> FrameCounter {
        self.step
    }
    /// The number of steps per second (Hz)
    pub fn rate(&self, frame_frame: f64) -> f64 {
        1.0 / (self.step as f64 * frame_frame)
    }
    /// The amount of frame left over from the last framestep
    pub fn remaining(&self) -> FrameCounter {
        self.accumulator
    }
    /// How much has the main game update "overstepped" the fixed framestep?
    /// (how many more (fractional) framesteps are left over in the accumulator)
    pub fn overstep(&self) -> u32 {
        self.accumulator - self.step
    }

    /// Pause the fixed framestep
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Un-pause (resume) the fixed framestep
    pub fn unpause(&mut self) {
        self.paused = false;
    }

    /// Toggle the paused state
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }
}

/// A Stage that runs a number of child stages with a fixed framestep
///
/// You can set the framestep duration. Every frame update, the frame delta
/// will be accumulated, and the child stages will run when it goes over
/// the framestep threshold. If multiple framesteps have been accumulated,
/// the child stages will be run multiple frames.
///
/// You can add multiple child stages, allowing you to use `Commands` in
/// your fixed framestep systems, and have their effects applied.
///
/// A good place to add the `FixedFramestepStage` is usually before
/// `CoreStage::Update`.
pub struct FixedFramestepStage {
    step: FrameCounter,
    accumulator: FrameCounter,
    paused: bool,
    label: FramestepName,
    stages: Vec<Box<dyn Stage>>,
    // rate_lock: (u32, f32),
    // lock_accum: u32,
}

impl FixedFramestepStage {
    /// Helper to create a `FixedFramestepStage` with a single child stage
    pub fn from_stage<S: Stage>(framestep: FrameCounter, label: FramestepName, stage: S) -> Self {
        Self::new(framestep, label).with_stage(stage)
    }

    /// Create a new empty `FixedFramestepStage` with no child stages
    pub fn new(framestep: FrameCounter, label: FramestepName) -> Self {
        Self {
            step: framestep,
            accumulator: FrameCounter::default(),
            paused: false,
            label,
            stages: Vec::new(),
            // rate_lock: (u32::MAX, 0.0),
            // lock_accum: 0,
        }
    }

    /// Builder method for starting in a paused state
    pub fn paused(mut self) -> Self {
        self.paused = true;
        self
    }

    /// Add a child stage
    pub fn add_stage<S: Stage>(&mut self, stage: S) {
        self.stages.push(Box::new(stage));
    }

    /// Builder method for adding a child stage
    pub fn with_stage<S: Stage>(mut self, stage: S) -> Self {
        self.add_stage(stage);
        self
    }

    /// ensure the FixedFramesteps resource exists and contains the latest data
    fn store_fixedframestepinfo(&self, world: &mut World) {
        if let Some(mut framesteps) = world.get_resource_mut::<FixedFramesteps>() {
            framesteps.current = Some(self.label);
            if let Some(mut info) = framesteps.info.get_mut(&self.label) {
                info.step = self.step;
                info.accumulator = self.accumulator;
                info.paused = self.paused;
            } else {
                framesteps.info.insert(self.label, FixedFramestepInfo {
                    step: self.step,
                    accumulator: self.accumulator,
                    paused: self.paused,
                });
            }
        } else {
            let mut framesteps = FixedFramesteps { current: Some(self.label),.. Default::default()};
            framesteps.info.insert(self.label, FixedFramestepInfo {
                step: self.step,
                accumulator: self.accumulator,
                paused: self.paused,
            });
            world.insert_resource(framesteps);
        }
    }
}

impl Stage for FixedFramestepStage {
    fn run(&mut self, world: &mut World) {
        if let Some(framesteps) = world.get_resource::<FixedFramesteps>() {
            if let Some(info) = framesteps.info.get(&self.label) {
                self.step = info.step;
                self.paused = info.paused;
                // do not sync accumulator
            }
        }

        if self.paused {
            return;
        }

        self.accumulator += 1;
        // {
        //     let frame = world.get_resource::<Frame>();
        //     if let Some(frame) = frame {
        //         frame.delta()
        //     } else {
        //         return;
        //     }
        // };


        let mut n_steps = 0;

        // while self.accumulator >= self.step {
        if self.accumulator == self.step {
            self.accumulator -= self.step;

            self.store_fixedframestepinfo(world);

            for stage in self.stages.iter_mut() {
                // run user systems
                stage.run(world);

                // if the user modified fixed framestep info, we need to copy it back
                if let Some(framesteps) = world.get_resource::<FixedFramesteps>() {
                    if let Some(info) = framesteps.info.get(&self.label) {
                        // update our actual step duration, in case the user has
                        // modified it in the info resource
                        self.step = info.step;
                        self.accumulator = info.accumulator;
                        self.paused = info.paused;
                    }
                }
            }
            n_steps += 1;
        }

        if let Some(mut framesteps) = world.get_resource_mut::<FixedFramesteps>() {
            framesteps.current = None;
        }

        if n_steps == 0 {
            self.store_fixedframestepinfo(world);
        }

        // if n_steps == 1 {
        //     if self.lock_accum < self.rate_lock.0 {
        //         self.lock_accum += 1;
        //     }
        //     if self.lock_accum >= self.rate_lock.0 {
        //         self.accumulator = self.step / 2;
        //     }
        // } else {
        //     self.lock_accum = 0;
        // }
    }
}

/// Type used as a Bevy Stage Label for fixed framestep stages
#[derive(Debug, Clone)]
pub struct FixedFrametepStageLabel(pub FramestepName);

impl StageLabel for FixedFrametepStageLabel {
    fn as_str(&self) -> &'static str {
        self.0
    }
}

/// Extensions to `bevy_app`
#[cfg(feature = "app")]
pub mod app {
    use bevy_ecs::prelude::*;
    use bevy_ecs::schedule::IntoSystemDescriptor;
    use bevy_app::{App, CoreStage};

    use super::{FixedFramestepStage, FixedFrametepStageLabel, FramestepName, FrameCounter};

    /// Extension trait with the methods to add to Bevy's `App`
    pub trait AppLooplessFixedFramestepExt {
        /// Create a new fixed framestep stage and add it to the schedule in the default position
        ///
        /// You need to provide a name string, which you can use later to do things with the framestep.
        ///
        /// The [`FixedFramestepStage`] is created with one child sub-stage: a Bevy parallel `SystemStage`.
        ///
        /// The new stage is inserted into the default position: before `CoreStage::Update`.
        fn add_fixed_framestep(&mut self, framestep: FrameCounter, label: FramestepName) -> &mut App;
        /// Create a new fixed framestep stage and add it to the schedule before a given stage
        ///
        /// Like [`add_fixed_framestep`], but you control where to add the fixed framestep stage.
        fn add_fixed_framestep_before_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut App;
        /// Create a new fixed framestep stage and add it to the schedule after a given stage
        ///
        /// Like [`add_fixed_framestep`], but you control where to add the fixed framestep stage.
        fn add_fixed_framestep_after_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut App;
        /// Add a child sub-stage to a fixed framestep stage
        ///
        /// It will be added at the end, after any sub-stages that already exist.
        ///
        /// The new stage will be a Bevy parallel `SystemStage`.
        fn add_fixed_framestep_child_stage(&mut self, framestep_name: FramestepName) -> &mut App;
        /// Add a custom child sub-stage to a fixed framestep stage
        ///
        /// It will be added at the end, after any sub-stages that already exist.
        ///
        /// You can provide any stage type you like.
        fn add_fixed_framestep_custom_child_stage(&mut self, framestep_name: FramestepName, stage: impl Stage) -> &mut App;
        /// Add a system to run under a fixed framestep
        ///
        /// To specify where to add the system, provide the name string of the fixed framestep, and the
        /// numeric index of the sub-stage (`0` if you have not added any additional sub-stages).
        fn add_fixed_framestep_system<Params>(&mut self, framestep_name: FramestepName, substage_i: usize, system: impl IntoSystemDescriptor<Params>) -> &mut App;
        /// Add many systems to run under a fixed framestep
        ///
        /// To specify where to add the systems, provide the name string of the fixed framestep, and the
        /// numeric index of the sub-stage (`0` if you have not added any additional sub-stages).
        fn add_fixed_framestep_system_set(&mut self, framestep_name: FramestepName, substage_i: usize, system_set: SystemSet) -> &mut App;
        /// Get access to the [`FixedFramestepStage`] for the fixed framestep with a given name string
        fn get_fixed_framestep_stage(&self, framestep_name: FramestepName) -> &FixedFramestepStage;
        /// Get mut access to the [`FixedFramestepStage`] for the fixed framestep with a given name string
        fn get_fixed_framestep_stage_mut(&mut self, framestep_name: FramestepName) -> &mut FixedFramestepStage;
        /// Get access to the i-th child sub-stage of the fixed framestep with the given name string
        fn get_fixed_framestep_child_substage<S: Stage>(&self, framestep_name: FramestepName, substage_i: usize) -> &S;
        /// Get mut access to the i-th child sub-stage of the fixed framestep with the given name string
        fn get_fixed_framestep_child_substage_mut<S: Stage>(&mut self, framestep_name: FramestepName, substage_i: usize) -> &mut S;
    }

    impl AppLooplessFixedFramestepExt for App {
        fn add_fixed_framestep(&mut self, framestep: FrameCounter, label: FramestepName) -> &mut App {
            self.add_fixed_framestep_before_stage(CoreStage::Update, framestep, label)
        }

        fn add_fixed_framestep_before_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut App {
            let ftstage = FixedFramestepStage::from_stage(framestep, label, SystemStage::parallel());
            ftstage.store_fixedframestepinfo(&mut self.world);
            self.add_stage_before(
                stage,
                FixedFrametepStageLabel(label),
                ftstage
            )
        }

        fn add_fixed_framestep_after_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut App {
            let ftstage = FixedFramestepStage::from_stage(framestep, label, SystemStage::parallel());
            ftstage.store_fixedframestepinfo(&mut self.world);
            self.add_stage_after(
                stage,
                FixedFrametepStageLabel(label),
                ftstage
            )
        }

        fn add_fixed_framestep_child_stage(&mut self, framestep_name: FramestepName) -> &mut App {
            let stage = self.schedule.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            stage.add_stage(SystemStage::parallel());
            self
        }

        fn add_fixed_framestep_custom_child_stage(&mut self, framestep_name: FramestepName, custom_stage: impl Stage) -> &mut App {
            let stage = self.schedule.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            stage.add_stage(custom_stage);
            self
        }

        fn add_fixed_framestep_system<Params>(&mut self, framestep_name: FramestepName, substage_i: usize, system: impl IntoSystemDescriptor<Params>) -> &mut App {
            let stage = self.schedule.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            let substage = stage.stages.get_mut(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_mut::<SystemStage>()
                .expect("Fixed Framestep sub-stage is not a SystemStage");
            substage.add_system(system);
            self
        }

        fn add_fixed_framestep_system_set(&mut self, framestep_name: FramestepName, substage_i: usize, system_set: SystemSet) -> &mut App {
            let stage = self.schedule.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            let substage = stage.stages.get_mut(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_mut::<SystemStage>()
                .expect("Fixed Framestep sub-stage is not a SystemStage");
            substage.add_system_set(system_set);
            self
        }

        fn get_fixed_framestep_stage(&self, framestep_name: FramestepName) -> &FixedFramestepStage {
            self.schedule.get_stage::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found")
        }

        fn get_fixed_framestep_stage_mut(&mut self, framestep_name: FramestepName) -> &mut FixedFramestepStage {
            self.schedule.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found")
        }

        fn get_fixed_framestep_child_substage<S: Stage>(&self, framestep_name: FramestepName, substage_i: usize) -> &S {
            let stage = self.get_fixed_framestep_stage(framestep_name);
            stage.stages.get(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_ref::<S>()
                .expect("Fixed Framestep sub-stage is not the requested type")
        }

        fn get_fixed_framestep_child_substage_mut<S: Stage>(&mut self, framestep_name: FramestepName, substage_i: usize) -> &mut S {
            let stage = self.get_fixed_framestep_stage_mut(framestep_name);
            stage.stages.get_mut(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_mut::<S>()
                .expect("Fixed Framestep sub-stage is not the requested type")
        }
    }
}

/// Extensions to Bevy Schedule
pub mod schedule {
    use bevy_ecs::prelude::*;
    use bevy_ecs::schedule::IntoSystemDescriptor;

    use super::{FixedFramestepStage, FixedFrametepStageLabel, FramestepName, FrameCounter};

    /// Extension trait with the methods to add to Bevy's `Schedule`
    pub trait ScheduleLooplessFixedFramestepExt {
        /// Create a new fixed framestep stage and add it to the schedule before a given stage
        ///
        /// You need to provide a name string, which you can use later to do things with the framestep.
        ///
        /// The [`FixedFramestepStage`] is created with one child sub-stage: a Bevy parallel `SystemStage`.
        ///
        /// Like [`add_fixed_framestep`], but you control where to add the fixed framestep stage.
        fn add_fixed_framestep_before_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut Schedule;
        /// Create a new fixed framestep stage and add it to the schedule after a given stage
        ///
        /// You need to provide a name string, which you can use later to do things with the framestep.
        ///
        /// The [`FixedFramestepStage`] is created with one child sub-stage: a Bevy parallel `SystemStage`.
        ///
        /// Like [`add_fixed_framestep`], but you control where to add the fixed framestep stage.
        fn add_fixed_framestep_after_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut Schedule;
        /// Add a child sub-stage to a fixed framestep stage
        ///
        /// It will be added at the end, after any sub-stages that already exist.
        ///
        /// The new stage will be a Bevy parallel `SystemStage`.
        fn add_fixed_framestep_child_stage(&mut self, framestep_name: FramestepName) -> &mut Schedule;
        /// Add a custom child sub-stage to a fixed framestep stage
        ///
        /// It will be added at the end, after any sub-stages that already exist.
        ///
        /// You can provide any stage type you like.
        fn add_fixed_framestep_custom_child_stage(&mut self, framestep_name: FramestepName, stage: impl Stage) -> &mut Schedule;
        /// Add a system to run under a fixed framestep
        ///
        /// To specify where to add the system, provide the name string of the fixed framestep, and the
        /// numeric index of the sub-stage (`0` if you have not added any additional sub-stages).
        fn add_fixed_framestep_system<Params>(&mut self, framestep_name: FramestepName, substage_i: usize, system: impl IntoSystemDescriptor<Params>) -> &mut Schedule;
        /// Add many systems to run under a fixed framestep
        ///
        /// To specify where to add the systems, provide the name string of the fixed framestep, and the
        /// numeric index of the sub-stage (`0` if you have not added any additional sub-stages).
        fn add_fixed_framestep_system_set(&mut self, framestep_name: FramestepName, substage_i: usize, system_set: SystemSet) -> &mut Schedule;
        /// Get access to the [`FixedFramestepStage`] for the fixed framestep with a given name string
        fn get_fixed_framestep_stage(&self, framestep_name: FramestepName) -> &FixedFramestepStage;
        /// Get mut access to the [`FixedFramestepStage`] for the fixed framestep with a given name string
        fn get_fixed_framestep_stage_mut(&mut self, framestep_name: FramestepName) -> &mut FixedFramestepStage;
        /// Get access to the i-th child sub-stage of the fixed framestep with the given name string
        fn get_fixed_framestep_child_substage<S: Stage>(&self, framestep_name: FramestepName, substage_i: usize) -> &S;
        /// Get mut access to the i-th child sub-stage of the fixed framestep with the given name string
        fn get_fixed_framestep_child_substage_mut<S: Stage>(&mut self, framestep_name: FramestepName, substage_i: usize) -> &mut S;
    }

    impl ScheduleLooplessFixedFramestepExt for Schedule {
        fn add_fixed_framestep_before_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut Schedule {
            self.add_stage_before(
                stage,
                FixedFrametepStageLabel(label),
                FixedFramestepStage::from_stage(framestep, label, SystemStage::parallel())
            )
        }

        fn add_fixed_framestep_after_stage(&mut self, stage: impl StageLabel, framestep: FrameCounter, label: FramestepName) -> &mut Schedule {
            self.add_stage_after(
                stage,
                FixedFrametepStageLabel(label),
                FixedFramestepStage::from_stage(framestep, label, SystemStage::parallel())
            )
        }

        fn add_fixed_framestep_child_stage(&mut self, framestep_name: FramestepName) -> &mut Schedule {
            let stage = self.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            stage.add_stage(SystemStage::parallel());
            self
        }

        fn add_fixed_framestep_custom_child_stage(&mut self, framestep_name: FramestepName, custom_stage: impl Stage) -> &mut Schedule {
            let stage = self.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            stage.add_stage(custom_stage);
            self
        }

        fn add_fixed_framestep_system<Params>(&mut self, framestep_name: FramestepName, substage_i: usize, system: impl IntoSystemDescriptor<Params>) -> &mut Schedule {
            let stage = self.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            let substage = stage.stages.get_mut(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_mut::<SystemStage>()
                .expect("Fixed Framestep sub-stage is not a SystemStage");
            substage.add_system(system);
            self
        }

        fn add_fixed_framestep_system_set(&mut self, framestep_name: FramestepName, substage_i: usize, system_set: SystemSet) -> &mut Schedule {
            let stage = self.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found");
            let substage = stage.stages.get_mut(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_mut::<SystemStage>()
                .expect("Fixed Framestep sub-stage is not a SystemStage");
            substage.add_system_set(system_set);
            self
        }

        fn get_fixed_framestep_stage(&self, framestep_name: FramestepName) -> &FixedFramestepStage {
            self.get_stage::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found")
        }

        fn get_fixed_framestep_stage_mut(&mut self, framestep_name: FramestepName) -> &mut FixedFramestepStage {
            self.get_stage_mut::<FixedFramestepStage>(
                FixedFrametepStageLabel(framestep_name)
            ).expect("Fixed Framestep Stage not found")
        }

        fn get_fixed_framestep_child_substage<S: Stage>(&self, framestep_name: FramestepName, substage_i: usize) -> &S {
            let stage = self.get_fixed_framestep_stage(framestep_name);
            stage.stages.get(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_ref::<S>()
                .expect("Fixed Framestep sub-stage is not the requested type")
        }

        fn get_fixed_framestep_child_substage_mut<S: Stage>(&mut self, framestep_name: FramestepName, substage_i: usize) -> &mut S {
            let stage = self.get_fixed_framestep_stage_mut(framestep_name);
            stage.stages.get_mut(substage_i)
                .expect("Fixed Framestep sub-stage not found")
                .downcast_mut::<S>()
                .expect("Fixed Framestep sub-stage is not the requested type")
        }
    }
}
