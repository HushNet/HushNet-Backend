FROM postgres:latest
ENV POSTGRES_USER=postgres
ENV POSTGRES_PASSWORD=dev
ENV POSTGRES_DB=e2ee
COPY sql_models/ /docker-entrypoint-initdb.d/
EXPOSE 5432
CMD ["postgres"]
