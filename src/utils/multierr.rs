use std::error::Error;
use std::fmt;

/// A container for multiple errors
#[derive(Debug)]
pub struct MultiError<E> {
    errors: Vec<E>,
}

impl<E> MultiError<E> {
    pub fn new(errors: Vec<E>) -> Self {
        Self { errors }
    }

    pub fn errors(&self) -> &[E] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<E> {
        self.errors
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl<E: fmt::Display> fmt::Display for MultiError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Multiple errors occurred ({}): ", self.errors.len())?;
        for (i, error) in self.errors.iter().enumerate() {
            if i > 0 {
                write!(f, "; ")?;
            }
            write!(f, "{}", error)?;
        }
        Ok(())
    }
}

impl<E: Error + 'static> Error for MultiError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.errors.first().map(|e| e as &dyn Error)
    }
}
