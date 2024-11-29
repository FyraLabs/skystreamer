FROM python:3.13.0

COPY . /app
WORKDIR /app

RUN pip install atproto
RUN pip install surrealdb

CMD ["python", "at_hose.py"]