//! Animation attributes become owned physical inputs through the actual
//! Skeleton dispatcher. This adapter loads stock settings and retains caches;
//! it does not choose animations or manufacture a push from controller input.
use skate_core::{
    animation::{
        output::{
            NativeMatrix,
            attributes::{AnimationAttribute, AttributeName},
        },
        skeleton_input::{
            attribute_finalization::{FinalizationInput, JumpAttributeState},
            contact_events::{ContactEventPose, ContactEventState},
            extended_attributes::ExtendedAttributes,
            name::encode,
            process_attributes,
            scalar_attributes::{AnimationControlOutput, ScalarAttributeInputs},
        },
    },
    input::controller::ActionMap,
};
use skate_data::{animation_frames::AnimationFrames, collections::Collections};

pub(crate) struct AnimationInput {
    pub fields: ScalarAttributeInputs,
    pub extra: ExtendedAttributes,
    pub contacts: ContactEventState,
    pub output: AnimationControlOutput,
    cached_jump: JumpAttributeState,
    bone_names: Vec<AttributeName>,
    right_toe: usize,
    settings: FinalizationInput,
    height_overrides: [bool; 5],
    /// AudibleFootStepStrength latched before the end-of-tick clear (SK-061).
    pub footsteps: AudibleFootsteps,
}

/// Footsteps marked by the stock OffBoard graph's AudibleFootStepStrength attribute
/// (SK-061): one per rising edge, counted so a reader in another schedule sees each once.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AudibleFootsteps {
    pub count: u32,
    pub strength: f32,
    tick_strength: f32,
}
impl AudibleFootsteps {
    pub fn record(&mut self, strength: f32) {
        if strength > 0.0 && self.tick_strength <= 0.0 {
            self.count = self.count.wrapping_add(1);
            self.strength = strength;
        }
        self.tick_strength = strength;
    }
}

impl AnimationInput {
    pub fn load(data: &Collections, frames: &AnimationFrames, mode: &str) -> Result<Self, String> {
        let right_toe = frames
            .bone_names
            .iter()
            .position(|n| n.eq_ignore_ascii_case("RightToeBase"))
            .ok_or("Stock skeleton is missing RightToeBase")?;
        Ok(Self {
            height_overrides: crate::difficulty::NATIVE_MODES.map(|key|
                data.boolean("physics_mode", key, "JumpHeightOverrideEnabled"))
                .into_iter().collect::<Result<Vec<_>, _>>()?.try_into().unwrap(),
            fields: ScalarAttributeInputs::reset(0, 0),
            extra: ExtendedAttributes::reset(0.0),
            contacts: ContactEventState {
                bone: 0,
                push_speed: 0.0,
            },
            cached_jump: JumpAttributeState::new(),
            // AnimOut construction82DB1E88..1EFC initializes the null name,
            // float24=0 and clears all represented high flag bits.
            output: AnimationControlOutput {
                grind_name: encode(b""),
                flags: 0,
            },
            bone_names: frames
                .bone_names
                .iter()
                .map(|n| encode(n.as_bytes()))
                .collect(),
            right_toe,
            footsteps: AudibleFootsteps::default(),
            settings: FinalizationInput {
                //TU3 global constructor8289F8B4 binds292 to anim_motion
                //collectionAC260B44FA3CA0A4 (jumping), not a default profile.
                select_jump_extremes: data.boolean("anim_motion", "jumping", "clamp_jump")?,
                low_jump_threshold: data.float("anim_motion", "jumping", "clamp_low_inclusive")?,
                high_jump_threshold: data.float(
                    "anim_motion",
                    "jumping",
                    "clamp_high_inclusive",
                )?,
                allow_height_override: data.boolean(
                    "physics_mode",
                    mode,
                    "JumpHeightOverrideEnabled",
                )?,
                use_prepared_controls: data.boolean(
                    "physics_jump",
                    "default",
                    "AdjustOnPrepare",
                )?,
                external_impulse_active: false,
                animation_flags: 0,
            },
        })
    }

    pub fn select_physics_mode(&mut self, mode: u32) -> Result<(), String> {
        self.settings.allow_height_override = *self.height_overrides.get(mode as usize)
            .ok_or_else(|| format!("Invalid animation physics mode {mode}"))?;
        Ok(())
    }

    /// ProcessedPhysIn Reset precedes physical/packet flag publication. The
    /// caller then publishes those flags before dispatching the current pose.
    pub fn reset_processed(&mut self) {
        self.fields = ScalarAttributeInputs::reset(self.fields.flags2468, self.fields.flags2488);
        self.extra = ExtendedAttributes::reset(self.extra.footstep_strength);
        self.contacts = ContactEventState {
            bone: 0,
            push_speed: 0.0,
        };
    }

    ///82DB7DCC..7E0C runs AFTER publishing AnimOut into the physical output.
    /// It is distinct from ProcessedPhysIn reset at the start of input.
    pub fn finish_output_publication(&mut self) {
        self.output.grind_name = encode(b"");
        self.output.flags &= 0x003fffff;
        self.footsteps.record(self.extra.footstep_strength);
        self.extra.footstep_strength = 0.0;
    }

    pub fn process(
        &mut self,
        attributes: &[AnimationAttribute],
        hierarchy: &[NativeMatrix],
        timestep: f32,
        animation_flags: u32,
        external_impulse_active: bool,
        map: &mut dyn ActionMap,
    ) -> Result<(), String> {
        let pose = ContactEventPose {
            bone_names: &self.bone_names,
            hierarchy,
            trajectory_bone: 0,
            right_toe_bone: self.right_toe as i32,
            timestep,
        };
        let settings = FinalizationInput {
            animation_flags,
            external_impulse_active,
            ..self.settings
        };
        process_attributes::process(
            attributes,
            &pose,
            &mut self.fields,
            &mut self.extra,
            &mut self.contacts,
            &mut self.cached_jump,
            &mut self.output,
            settings,
            Some(map),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::AudibleFootsteps;

    #[test]
    fn footsteps_count_each_marked_plant_once() {
        // AudibleFootStepStrength per tick: a plant held 3 ticks, a one-tick plant, then a
        // plant right after a gap of one tick.
        let curve = [0.0, 0.8, 0.8, 0.8, 0.0, 0.0, 0.5, 0.0, 1.0, 1.0];
        let mut steps = AudibleFootsteps::default();
        let counts: Vec<u32> = curve.iter().map(|&s| { steps.record(s); steps.count }).collect();
        assert_eq!(counts, [0, 1, 1, 1, 1, 1, 2, 2, 3, 3]);
        assert_eq!(steps.strength, 1.0);
    }
}
