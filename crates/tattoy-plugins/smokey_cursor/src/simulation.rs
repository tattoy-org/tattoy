//! All the maths to do a smoke simulation
//! Heavily inspired by [mueller-sph-rs](https://github.com/lucas-schuermann/mueller-sph-rs)

use rayon::iter::{IntoParallelRefMutIterator as _, ParallelIterator as _};
use std::collections::VecDeque;

use glam::Vec2;

use super::{
    config::Config,
    particle::{Particle, PARTICLE_SIZE_SQUARED},
};

/// Number of times to iterate the simulation per graphical frame
const NUMBER_OF_SIMULATION_STEPS_PER_TICK: usize = 5;

/// The main code for the simulation, manages the `tick` etc
#[derive(Default)]
#[non_exhaustive]
pub struct Simulation {
    /// Width of the simulation
    pub width: f32,
    /// Height of the simulation (double the rows of the TTY)
    pub height: f32,
    /// All the particles
    pub particles: VecDeque<Particle>,
    /// All the particles as spatially-optimised neighbours
    pub neighbours: rstar::RTree<Particle>,
    /// The configurable settings for the simulation
    pub config: Config,
}

#[expect(
    clippy::cast_precision_loss,
    clippy::as_conversions,
    clippy::arithmetic_side_effects,
    clippy::float_arithmetic,
    reason = "This is a prototype"
)]
impl Simulation {
    /// Initialise a new simulation
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        let config = Config {
            initial_velocity: Vec2::new(0.01, -0.1).into(),
            ..Default::default()
        };
        Self {
            width: width as f32 * config.scale * super::particle::PARTICLE_SIZE,
            height: height as f32 * config.scale * super::particle::PARTICLE_SIZE,
            particles: VecDeque::default(),
            neighbours: rstar::RTree::new(),
            config,
        }
    }

    /// Is the simulation ready?
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }

    /// A tick of a graphical frame render
    pub fn tick(&mut self, cursor: (u16, u16), cells: &Vec<tattoy_protocol::Cell>) {
        if crate::is_random_trigger(1) {
            self.add_particle(f32::from(cursor.0), f32::from(cursor.1 * 2));
        }

        let pty_pixel_count = self.add_pty_particles(cursor, cells);

        for _ in 0..NUMBER_OF_SIMULATION_STEPS_PER_TICK {
            self.evolve();
        }

        for _ in 0..pty_pixel_count {
            self.particles.pop_front();
        }

        self.remove_old_particles();
    }

    /// Step through the simulation
    fn evolve(&mut self) {
        self.build_neighbours_lookup();
        self.compute_density_pressure();
        self.compute_forces();
        self.integrate();
    }

    /// Copy all the particles into a fast spatial lookup tree
    fn build_neighbours_lookup(&mut self) {
        let mut neighbours = Vec::new();
        for particle in &mut self.particles {
            neighbours.push(particle.clone());
        }
        self.neighbours = rstar::RTree::bulk_load(neighbours);
    }

    /// Calculate the next position of the particles
    fn integrate(&mut self) {
        for particle in &mut self.particles {
            particle.integrate();
            particle.boundaries(self.width, self.height);
        }
    }

    /// Calculate the density and pressure affecting the particles
    fn compute_density_pressure(&mut self) {
        self.particles.par_iter_mut().for_each(|particle| {
            particle.density = 0.0;

            // TODO: cache?
            let neighbours = self.neighbours.locate_within_distance(
                [particle.position.x, particle.position.y],
                PARTICLE_SIZE_SQUARED,
            );

            neighbours.for_each(|neighbour| {
                particle.accumulate_density(neighbour);
            });

            particle.update_pressure();
        });
    }

    /// Compute forces on the particles, from density, pressure and gravity
    fn compute_forces(&mut self) {
        self.particles.par_iter_mut().for_each(|particle| {
            particle.force = Vec2::ZERO;

            // TODO: cache?
            let neighbours = self.neighbours.locate_within_distance(
                [particle.position.x, particle.position.y],
                PARTICLE_SIZE_SQUARED,
            );

            neighbours.for_each(|neighbour| {
                if particle.position == neighbour.position {
                    return;
                }

                if let Some(density_and_pressure) = particle.calculate_forces(neighbour) {
                    particle.force += density_and_pressure;
                }
            });

            let gravity = particle.force_from_gravity(self.config.gravity.into());
            particle.force += gravity;
        });
    }
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::float_cmp,
    reason = "Tests aren't so strict"
)]
mod test {
    use super::*;

    fn make_sim() -> Simulation {
        let mut sim = Simulation::new(100, 100);
        sim.config.gravity = Vec2::ZERO.into();
        sim.config.initial_velocity = Vec2::ZERO.into();
        sim.config.scale = 1.0 * crate::particle::PARTICLE_SIZE; // So we don't have to scale/unscale
        sim
    }

    fn add_particle(sim: &mut Simulation, position: Vec2) {
        let particle = Particle {
            position,
            ..Default::default()
        };
        sim.particles.push_front(particle);
    }

    #[test]
    fn basic() {
        let mut sim = Simulation::new(100, 100);
        let pty = Vec::<tattoy_protocol::Cell>::new();
        for _ in 0usize..10 {
            sim.tick((50, 50), &pty);
        }
        assert!(sim.particles.len() > 5);
        assert!(sim.neighbours.size() > 5);
    }

    #[test]
    fn distant_particles_dont_interact() {
        let mut sim = make_sim();
        add_particle(&mut sim, Vec2::new(0.0, 0.0));
        add_particle(&mut sim, Vec2::new(99.0, 99.0));
        for _ in 0usize..100 {
            sim.evolve();
        }
        assert_eq!(sim.particles[1].position, Vec2::new(0.0, 0.0));
        assert_eq!(sim.particles[0].position_unscaled(), Vec2::new(99.0, 99.0));
    }

    #[test]
    fn extremely_close_particles_repel() {
        let mut sim = make_sim();
        add_particle(&mut sim, Vec2::new(50.0, 50.0));
        add_particle(&mut sim, Vec2::new(55.0, 55.0));

        let distance_before = sim.particles[0]
            .position
            .distance(sim.particles[1].position);
        for _ in 0usize..100 {
            sim.evolve();
        }
        let distance_after = sim.particles[0]
            .position
            .distance(sim.particles[1].position);

        assert!(
            distance_before < distance_after,
            "before/after: {distance_before:?}/{distance_after:?}"
        );
    }

    #[test]
    fn gravity_moves_particle() {
        let mut sim = make_sim();
        sim.config.gravity = Vec2::new(0.0, -1.0).into();
        add_particle(&mut sim, Vec2::new(50.0, 50.0));

        for _ in 0usize..10 {
            sim.evolve();
        }

        let x = sim.particles[0].position.x;
        let y = sim.particles[0].position.y;
        assert!(y < 50.0, "y: {y}");
        assert!(y > 40.0, "y: {y}");
        assert_eq!(x, 50.0);
    }
}
