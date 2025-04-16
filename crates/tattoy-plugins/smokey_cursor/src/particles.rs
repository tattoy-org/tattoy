//! Functions that add and remove particles

use rand::Rng as _;
use std::ops::Div as _;

use glam::Vec2;

use super::{
    particle::{Particle, PARTICLE_SIZE},
    simulation::Simulation,
};

/// The number of attempts allowed to try to find a safe place to add a new particle
const ATTEMPTS_TO_FIND_SAFE_PLACE: usize = 100;

#[expect(
    clippy::cast_precision_loss,
    clippy::as_conversions,
    clippy::arithmetic_side_effects,
    clippy::float_arithmetic,
    reason = "We're just prototyping for now"
)]
impl Simulation {
    /// Add particles that represent the character from the user's current TTY. These particles are
    /// immovable, yet they still interact with the simulation.
    ///
    /// # Panics
    /// When it can't safely cast the cursor usize to i32
    pub fn add_pty_particles(
        &mut self,
        cursor: (u16, u16),
        pty: &Vec<tattoy_protocol::Cell>,
    ) -> usize {
        let scale = self.config.scale * super::particle::PARTICLE_SIZE;
        let mut count: usize = 0;

        for cell in pty {
            if !cell.character.is_whitespace() {
                let x_f32 = cell.coordinates.0 as f32;
                let y_f32 = cell.coordinates.1 as f32;

                let pty_particl1 = Particle::default_immovable(scale, x_f32, y_f32);
                self.particles.push_front(pty_particl1.clone());
                count += 1;

                let pty_particle2 = Particle::default_immovable(scale, x_f32, y_f32 + 1.0);
                self.particles.push_front(pty_particle2.clone());
                count += 1;
            }
        }

        // Surround the cursor with particles so that the cursor can interact with the other
        // particles.
        let radius: i32 = 8;
        for y in 0i32..radius {
            for x in 0i32..radius {
                let cursor_x: i32 = cursor.0.into();
                let cursor_y: i32 = cursor.1.into();
                let x_f32 = (cursor_x + x - (radius.div(2i32))) as f32;
                let y_f32 = (cursor_y * 2i32 + y - (radius.div(2i32))) as f32;
                let mut cursor_particle = Particle::default_immovable(scale, x_f32, y_f32 + 1.0);
                cursor_particle.is_immovable = false;
                self.particles.push_front(cursor_particle.clone());
                count += 1;
            }
        }
        count
    }

    /// Remove first-in particles from FILO queue
    pub fn remove_old_particles(&mut self) {
        if self.particles.len() < self.config.max_particles {
            return;
        }
        self.particles.pop_back();
    }

    /// Safely add a particle without creating "explosions"
    pub fn add_particle(&mut self, x: f32, y: f32) {
        if let Some((x_safe, y_safe)) = self.find_safe_place(x, y) {
            let particle = Particle::default_movable(
                self.config.scale * super::particle::PARTICLE_SIZE,
                self.config.initial_velocity.into(),
                x_safe,
                y_safe,
            );
            self.particles.push_front(particle);
        }
    }

    /// Based on the requested location of the new particle find a position near it, but also a
    /// safe distance from other particles, so as not to create unrealistic "explosive" responses.
    fn find_safe_place(&self, mut x: f32, mut y: f32) -> Option<(f32, f32)> {
        if self.particles.is_empty() {
            return Some((x, y));
        }

        let mut too_close;
        for _ in 0usize..ATTEMPTS_TO_FIND_SAFE_PLACE {
            too_close = false;
            for particle in &self.particles {
                let delta = particle.position - Vec2::new(x, y);
                let distance = delta.length();
                if distance < PARTICLE_SIZE {
                    too_close = true;
                    x += rand::thread_rng().gen_range(-PARTICLE_SIZE..PARTICLE_SIZE);
                    y += rand::thread_rng().gen_range(-PARTICLE_SIZE..PARTICLE_SIZE);
                    break;
                }
            }

            if !too_close {
                return Some((x, y));
            }
        }

        None
    }
}
